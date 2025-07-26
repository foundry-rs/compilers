macro_rules! vyper_node {
    (
        $(#[$struct_meta:meta])*
        struct $name:ident {
            $(
                $(#[$field_meta:meta])*
                $field:ident: $ty:ty
            ),* $(,)?
        }
    ) => {
        $(#[$struct_meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $name {
            pub node_id: i32,
            pub src: String,
            pub lineno: u32,
            pub col_offset: u32,
            pub end_lineno: u32,
            pub end_col_offset: u32,
            $(
                $(#[$field_meta])*
                pub $field: $ty
            ),*
        }
    };
}

macro_rules! basic_vyper_nodes {
    (
        $(
            $(#[$struct_meta:meta])*
            $name:ident
        ),* $(,)?
    ) => {
        $(
            vyper_node! {
                $(#[$struct_meta])*
                struct $name {}
            }
        )*
    }
}

macro_rules! node_group {
    (
        $group:ident;

        $(
            $(#[$field_meta:meta])?
            $name:ident
        ),* $(,)?
    ) => {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "ast_type")]
        pub enum $group {
            $(
                $(#[$field_meta])*
                $name($name),
            )*
        }
    };
}

pub(crate) use vyper_node;
pub(crate) use basic_vyper_nodes;
pub(crate) use node_group;
