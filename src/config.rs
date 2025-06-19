// Name of the table which contains all the elements
pub(crate) static ELEMENTS_TABLE: &str = "elements";

// Name of the table which contains all element to element relations
pub(crate) static RELATIONS_TABLE: &str = "relations";

// Name of the table which contains other 1:n properties for an element
pub(crate) static EXTENDED_TABLE: &str = "extended_properties";

// Name of the column which contains the pimary key
pub(crate) const ELEMENT_PK_COL: &str = "@id";

// Name of known polymorphic properties
pub(crate) const POLYMORPHIC_PROPS: [&str; 1] = ["value"];

/// Minimum time interval inbetween status reports
pub(crate) const TIME_BETWEEN_STATUS_REPORTS: std::time::Duration =
    std::time::Duration::from_secs(5);
