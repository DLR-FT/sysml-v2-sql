#[derive(Default, Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    #[serde(rename = "@id")]
    pub id: String,
    // #[serde(rename = "@type")]
    // pub type_field: String,
    pub created: String,
    pub default_branch: DefaultBranch,
    pub description: Option<String>,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultBranch {
    #[serde(rename = "@id")]
    pub id: String,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    #[serde(rename = "@id")]
    pub id: String,
    // #[serde(rename = "@type")]
    // pub type_field: String,
    pub created: String,
    pub head: BranchHead,
    pub name: String,
    pub owning_project: OwningProject,
    pub referenced_commit: ReferencedCommit,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchHead {
    #[serde(rename = "@id")]
    pub id: String,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwningProject {
    #[serde(rename = "@id")]
    pub id: String,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferencedCommit {
    #[serde(rename = "@id")]
    pub id: String,
}
