use crate::{
    cli::{CommitSelector, ProjectSelector},
    import::Element,
    maybe_time_report,
};
use std::{
    collections::HashMap,
    fs::File,
    path::PathBuf,
    sync::atomic::Ordering::Relaxed,
    sync::{atomic::AtomicUsize, Arc},
};

use api_data_types::{Branch, Project};
use eyre::{bail, ensure, Result};
use reqwest::{Client, RequestBuilder, Response, Url};
use tokio::task::JoinHandle;

mod api_data_types;

pub struct SysmlV2ApiBrowser {
    base_url: Url,

    maybe_username: Option<String>,
    maybe_password: Option<String>,

    http_client: Client,
}

impl SysmlV2ApiBrowser {
    pub fn new(base_url: Url, allow_invalid_certs: bool) -> Result<Self> {
        ensure!(
            !base_url.path().ends_with('/'),
            "base_url must not end with /"
        );

        let http_client;

        #[cfg(any(feature = "bundled-tls", feature = "native-tls"))]
        {
            if allow_invalid_certs {
                warn!("accepting invalid certificates, connection to server is NOT trustworthy");
            }
            http_client = Client::builder().danger_accept_invalid_certs(allow_invalid_certs);
        }

        #[cfg(not(any(feature = "bundled-tls", feature = "native-tls")))]
        {
            http_client = Client::builder();
            if allow_invalid_certs {
                warn!("-a/--allow-invalid-certs is ignored since no TLS support was compiled in at all");
            }
        }

        let http_client = http_client.build()?;

        let maybe_username = match std::env::var("SYSML_USERNAME") {
            Err(std::env::VarError::NotPresent) => None,
            maybe_u => Some(maybe_u?),
        };

        let maybe_password = match std::env::var("SYSML_PASSWORD") {
            Err(std::env::VarError::NotPresent) => None,
            maybe_p => Some(maybe_p?),
        };

        Ok(Self {
            base_url,
            maybe_username,
            maybe_password,
            http_client,
        })
    }

    fn absolute_url<S: AsRef<str>>(&self, url_path: S) -> Url {
        let mut url = self.base_url.clone();
        if url_path.as_ref().starts_with('/') {
            url.set_path(url_path.as_ref());
        } else {
            let previous_path = url.path();
            url.set_path(&format!("{previous_path}/{}", url_path.as_ref()));
        }
        url
    }

    fn maybe_set_auth(&self, req: RequestBuilder) -> Result<RequestBuilder> {
        let req = match (&self.maybe_username, &self.maybe_password) {
            (None, None) => req,
            (None, Some(_)) => {
                bail!("when specifying a password, a username must be provide as well")
            }
            (Some(username), maybe_password) => req.basic_auth(username, maybe_password.clone()),
        };

        Ok(req)
    }

    async fn http_get<T: reqwest::IntoUrl + std::fmt::Display>(&self, url: T) -> Result<Response> {
        trace!("about to get {url}");

        // prepare the request
        let req = self.http_client.get(url);

        // optionally add auth
        let req = self.maybe_set_auth(req)?;

        // perform the request
        req.send().await.map_err(|e| e.into())
    }
}

/// Interprete the CLI arguments, finding the matching project and commit id
///
/// Lookup by name looks at the start of the project name, e.g. a project named 'My Project' will
/// match already for the searched name 'My', if there are no other projects by that name
pub async fn interprete_cli(
    browser: &SysmlV2ApiBrowser,
    project_selector: &ProjectSelector,
) -> Result<(String, String)> {
    let mut maybe_matched_project = None;

    let (project_id, commit_selector) = match project_selector {
        ProjectSelector::ProjectId { project_id, commit } => {
            debug!("picking project_id {project_id:?}");
            (project_id.to_owned(), commit)
        }
        ProjectSelector::ProjectName {
            project_name,
            commit,
        } => {
            debug!("searching for project by the name {project_name:?}");

            let url = browser.absolute_url("projects");
            let projects: Vec<Project> = browser.http_get(url).await?.json().await?;

            trace!("found the following projects:\n{projects:#?}");

            let matching_projects: Vec<&Project> = projects
                .iter()
                .filter(|p| p.name.starts_with(project_name))
                .collect();

            if matching_projects.is_empty() {
                error!("no project matched the specified name {project_name:?}");
                info!("the following project where found:\n{projects:#?}");
                bail!("error finding any matching project");
            } else if matching_projects.len() > 1 {
                error!("multiple projects matched the specified name {project_name:?}, please be more specific");
                info!("the following projects where found:\n{matching_projects:#?}");
                bail!("error finding exactly one matching project");
            }

            maybe_matched_project = Some(matching_projects[0].to_owned());

            (matching_projects[0].id.to_owned(), commit)
        }
    };

    let commit_id = match commit_selector {
        CommitSelector::CommitId { commit_id } => commit_id.to_owned(),
        CommitSelector::BranchId { branch_id } => {
            let url = browser.absolute_url(format!("projects/{project_id}/branches/{branch_id}"));
            let branch: Branch = browser.http_get(url).await?.json().await?;
            branch.head.id.to_owned()
        }
        CommitSelector::BranchName { branch_name } => {
            debug!("searching for branch by the name {branch_name:?}");

            let url = browser.absolute_url(format!("projects/{project_id}/branches"));
            let branches: Vec<Branch> = browser.http_get(url).await?.json().await?;

            trace!("found the following branches:\n{branches:#?}");

            let matching_branches: Vec<&Branch> = branches
                .iter()
                .filter(|p| p.name.starts_with(branch_name))
                .collect();

            if matching_branches.is_empty() {
                error!("no branch matched the specified name {branch_name:?}");
                info!("the following branch where found:\n{branches:#?}");
                bail!("error finding any matching branch");
            } else if matching_branches.len() > 1 {
                error!("multiple branches matched the specified name {branch_name:?}, please be more specific");
                info!("the following branches where found:\n{matching_branches:#?}");
                bail!("error finding exactly one matching branch");
            }

            matching_branches.first().unwrap().head.id.to_owned()
        }
        CommitSelector::DefaultBranch => {
            let matched_project = match maybe_matched_project {
                Some(matched_project) => matched_project.to_owned().to_owned(),
                None => {
                    let url = browser.absolute_url(format!("projects/{project_id}"));
                    let project: Project = browser.http_get(url).await?.json().await?;
                    project
                }
            };

            let url = browser.absolute_url(format!(
                "projects/{project_id}/branches/{}",
                matched_project.default_branch.id
            ));
            let default_branch: Branch = browser.http_get(url).await?.json().await?;

            default_branch.head.id
        }
    };

    Ok((project_id, commit_id))
}

/// Checks if there are [`Element`]s with conflicting values
///
/// The SysML v2 API works with Arrays of [`Element`]s, which have a unique id. A faulty yet
/// representable situation would be having multiple, differing [`Element`]s with the same id. This
/// function scans  for this issue, and yields an error if at least one occurence is found.
pub fn check_for_conflicting_elements<'a>(
    elements: &'a mut [Element],
    element_id_idx_map: &mut HashMap<&'a str, usize>,
) -> Result<()> {
    let now = std::time::Instant::now();
    debug!("checking for coflicting elements");
    // We need to check that there are no duplicate elements with the same id in the dataset
    for (idx, new_element) in elements.iter().enumerate() {
        match element_id_idx_map.get(new_element.id.as_str()) {
            // existing element is identical to new_element, all good
            Some(existing_element_idx) if &elements[*existing_element_idx] == new_element => {}

            // existing element is **not** identical, this is an issue
            Some(existing_element) => {
                bail!("Differing Elements with colliding ids where found:\n{existing_element:#?}\n{new_element:#?}");
            }

            // no existing element
            None => {
                element_id_idx_map.insert(&new_element.id, idx);
            }
        }
    }
    debug!(
        "no conflicting elements where found, check took {:?}",
        now.elapsed()
    );

    Ok(())
}

pub fn build_url_path(project_id: &str, commit_id: &str, maybe_page_size: Option<u32>) -> String {
    let mut url = format!("projects/{project_id}/commits/{commit_id}/elements");

    if let Some(page_size) = maybe_page_size {
        url += &format!("?page[size]={page_size}");
    }
    url
}

/// # Overview
///
/// Fetches all data from `base_url`,
pub async fn fetch_from_url_to_file(
    browser: SysmlV2ApiBrowser,
    url_path: &str,
    maybe_path: &Option<PathBuf>,
    maybe_conn: Option<&mut rusqlite::Connection>,
    pretty_json: bool,
    importer_config: crate::import::ImporterConfiguration,
) -> Result<()> {
    let fetch_t0 = std::time::Instant::now();

    let mut element_id_idx_map: HashMap<_, usize> = HashMap::new();
    if let Some(path) = maybe_path {
        if path.is_file() {
            info!("{path:?} exists and is a file, appending to it");
            let mut elements: Vec<_> = crate::util::read_json_file(path)?;
            check_for_conflicting_elements(&mut elements, &mut element_id_idx_map)?;
        }
    }
    info!("fetching started");

    let now = std::time::Instant::now();

    // channel to move responses from the http task to the deser task
    let (resp_tx, mut resp_rx) = tokio::sync::mpsc::channel::<Response>(32);

    // performance counters
    let elements_count = Arc::new(AtomicUsize::new(0));
    let pages_count = Arc::new(AtomicUsize::new(0));

    // this task receives `reqwest::Response`s and parses their bodies JSON
    let elements_count_clone = elements_count.clone();
    let json_deser_task: JoinHandle<Result<Vec<Element>>> = tokio::task::spawn(async move {
        let mut elements: Vec<Element> = Vec::new();
        while let Some(resp) = resp_rx.recv().await {
            trace!("parsing new response body");
            let mut new_elements: Vec<Element> = resp.json().await?;

            if new_elements.is_empty() {
                warn!("detectected empty page, terminating parser task");
                break;
            }

            elements.append(&mut new_elements);
            elements_count_clone.store(elements.len(), Relaxed);
        }

        Ok(elements)
    });

    let mut maybe_url = Some(browser.absolute_url(url_path));

    // this task pulls the next page until there is no next page
    let pages_count_clone = pages_count.clone();
    let http_paginator_task: JoinHandle<Result<()>> = tokio::task::spawn(async move {
        while let Some(url) = maybe_url.take() {
            // send request and gather response
            trace!("sending new request to {url}");
            let resp = browser.http_get(url).await?;

            // if there is a next page, make sure we get to it in the next iteration
            'next_page_exists: {
                let Some(link_header) = resp.headers().get(reqwest::header::LINK) else {
                    break 'next_page_exists;
                };
                let link_headers = parse_link_header::parse_with_rel(link_header.to_str()?)?;

                trace!("found the following headers in the current page\n{link_headers:#?}");

                let Some(next_url) = link_headers.get("next") else {
                    break 'next_page_exists;
                };
                let next_url = Url::parse(&next_url.raw_uri)?;
                trace!("next url to be processed: {next_url:#?}");
                maybe_url = Some(next_url);
            }

            // submit the response to the json parser task
            if resp_tx.send(resp).await.is_err() {
                trace!("deser_task dropped resp_rx, shutting down");
                break;
            }

            // and count the pages we processed
            pages_count_clone.fetch_add(1, Relaxed);
        }

        Ok(())
    });

    // this task just montitors the progress of the other tasks
    let mut report_td = std::time::Duration::from_secs(0);
    let elements_count_clone = elements_count.clone();
    let monitor_task = tokio::task::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            maybe_time_report!(
                "element",
                fetch_t0,
                report_td,
                elements_count_clone.load(Relaxed)
            );
        }
    });

    http_paginator_task.await??;
    let elements = json_deser_task.await??;
    monitor_task.abort();

    info!(
        "fetched {} elements spread over {} pages in {:?}",
        elements.len(),
        pages_count.load(Relaxed),
        now.elapsed()
    );

    // TODO maybe deduplicate

    if let Some(path) = maybe_path {
        info!("writing the fetched data to {path:?}");
        let f = File::create(path)?;
        if pretty_json {
            serde_json::to_writer_pretty(f, &elements)?;
        } else {
            serde_json::to_writer(f, &elements)?;
        }
    }

    // deduplicate_elements(&mut elements, &mut element_id_idx_map)?;

    if let Some(conn) = maybe_conn {
        crate::import::import_from_slice(&elements, conn, &importer_config)?;
    }

    Ok(())
}
