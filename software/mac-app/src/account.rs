//! Identity comes only from official account/read, never auth files or JWT claims.
use crate::harness::Origin;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub key: String,
    pub label: String,
    pub verified: bool,
    pub device: Origin,
    pub plan: String,
}
impl Identity {
    pub fn from_official(value: Option<&Value>, root: &Path) -> Option<Self> {
        Self::parse(value, Origin::local(root).ok()?)
    }
    pub(crate) fn parse(value: Option<&Value>, device: Origin) -> Option<Self> {
        let account = value.map(|v| &v["account"]);
        let plan = account.and_then(|v| v["planType"].as_str()).unwrap_or("");
        let plan = if matches!(
            plan,
            "free" | "go" | "plus" | "pro" | "team" | "business" | "enterprise" | "edu"
        ) {
            plan.to_owned()
        } else {
            String::new()
        };
        let email = account
            .filter(|a| a["type"] == "chatgpt")
            .and_then(|a| a["email"].as_str())
            .map(str::trim)
            .filter(|s| {
                s.len() <= 254
                    && s.is_ascii()
                    && !s
                        .bytes()
                        .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
            })
            .and_then(|s| {
                s.split_once('@')
                    .filter(|(a, b)| !a.is_empty() && !b.is_empty() && !b.contains('@'))
                    .map(|(a, _)| (s, a))
            });
        let (key, label, verified) = if let Some((email, local)) = email {
            let mut hash = Sha256::new();
            hash.update(b"HomeCraft/codex-account/v1\0");
            hash.update(email.to_ascii_lowercase().as_bytes());
            let key = format!("{:x}", hash.finalize());
            let prefix = local
                .chars()
                .take(3.min(local.len().saturating_sub(1).max(1)))
                .collect::<String>();
            (key, format!("{prefix}***"), true)
        } else {
            (format!("device-{}", device.id), device.name.clone(), false)
        };
        let identity = Self {
            key,
            label,
            verified,
            device,
            plan,
        };
        identity.valid().then_some(identity)
    }
    pub fn valid(&self) -> bool {
        self.device.valid()
            && self.plan.len() <= 16
            && self.plan.bytes().all(|c| c.is_ascii_lowercase())
            && if self.verified {
                self.key.len() == 64
                    && self
                        .key
                        .bytes()
                        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                    && self.label.strip_suffix("***").is_some_and(|s| {
                        !s.is_empty()
                            && s.len() <= 3
                            && s.bytes().all(|b| b.is_ascii_graphic() && b != b'@')
                    })
            } else {
                self.key == format!("device-{}", self.device.id) && self.label == self.device.name
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn origin(id: &str) -> Origin {
        Origin {
            id: id.into(),
            name: "Mac-1".into(),
        }
    }
    #[test]
    fn official_email_is_masked_and_same_account_has_same_key_on_two_macs() {
        let value =
            json!({"account":{"type":"chatgpt","email":"sample@example.test","planType":"plus"}});
        let a = Identity::parse(Some(&value), origin("1234567890abcdef")).unwrap();
        let b = Identity::parse(Some(&value), origin("abcdef1234567890")).unwrap();
        assert_eq!(a.key, b.key);
        assert_eq!(a.label, "sam***");
        let text = serde_json::to_string(&a).unwrap();
        assert!(!text.contains("sample@"));
        assert!(!text.contains("example.test"));
        assert!(a.verified);
    }
    #[test]
    fn non_chatgpt_null_and_invalid_email_fall_back_to_device() {
        for value in [
            json!(null),
            json!({"account":{"type":"apiKey","email":"x@y"}}),
            json!({"account":{"type":"chatgpt","email":null}}),
            json!({"account":{"type":"chatgpt","email":"a@b@c"}}),
        ] {
            let id = Identity::parse(Some(&value), origin("1234567890abcdef")).unwrap();
            assert!(!id.verified);
            assert_eq!(id.label, "Mac-1");
        }
    }
    #[test]
    fn changing_plan_does_not_duplicate_the_same_email() {
        let mut value =
            json!({"account":{"type":"chatgpt","email":"sample@example.test","planType":"plus"}});
        let a = Identity::parse(Some(&value), origin("1234567890abcdef")).unwrap();
        value["account"]["planType"] = json!("pro");
        let b = Identity::parse(Some(&value), origin("1234567890abcdef")).unwrap();
        assert_eq!(a.key, b.key);
        assert_ne!(a.plan, b.plan);
    }
}
