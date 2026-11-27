/// An enum of unit variants `config.toml` names by the strings given: one
/// spelling serves `key` and `Deserialize` both.
macro_rules! written {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident = $key:literal,)+
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$variant_meta])* $variant,)+
        }

        impl $name {
            /// How `config.toml` spells it.
            pub const fn key(self) -> &'static str {
                match self {
                    $(Self::$variant => $key,)+
                }
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let written = <String as serde::Deserialize>::deserialize(deserializer)?;
                [$(Self::$variant,)+]
                    .into_iter()
                    .find(|value| value.key() == written)
                    .ok_or_else(|| serde::de::Error::unknown_variant(&written, &[$($key,)+]))
            }
        }
    };
}

pub(crate) use written;
