mod _util;
use _util::fake_enum_impls;

#[derive(serde::Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyLevel {
    Public,
    Private,
}

fake_enum_impls!(PrivacyLevel);

impl From<i32> for PrivacyLevel {
    fn from(value: i32) -> Self {
        match value {
            1 => PrivacyLevel::Public,
            2 => PrivacyLevel::Private,
            _ => unreachable!(),
        }
    }
}

macro_rules! model {
    ($n:ident) => {
        mod $n;
        pub use $n::*;
    };
}

model!(authnz);
model!(oauth2_app);
model!(system);
model!(system_config);
