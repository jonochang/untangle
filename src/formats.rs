use clap::ValueEnum;
use serde::Serialize;

macro_rules! format_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, ValueEnum, Serialize, PartialEq, Eq)]
        #[serde(rename_all = "lowercase")]
        pub enum $name {
            $($variant),+
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$variant => write!(f, "{}", stringify!($variant).to_lowercase())),+
                }
            }
        }
    };
}

format_enum!(AnalyzeReportFormat { Json, Text, Sarif });
format_enum!(GraphFormat { Json, Dot });
format_enum!(ArchitectureFormat { Json, Dot });
format_enum!(ArchitectureCheckFormat { Json, Text });
format_enum!(DiffFormat { Json, Text });
format_enum!(QualityFormat { Json, Text });
format_enum!(ServiceGraphFormat { Json, Text, Dot });
