//! Visible feedback for explicit actions; automatic background polling stays quiet.
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
pub struct Feedback {
    pub message: String,
    pub busy: bool,
    pub error: bool,
    pub at: i64,
}
impl Feedback {
    pub fn new(message: impl Into<String>, busy: bool, error: bool) -> Self {
        Self {
            message: message.into(),
            busy,
            error,
            at: crate::usage::now(),
        }
    }
    pub fn visible(&self, now: i64) -> bool {
        now.saturating_sub(self.at) < if self.busy { 180 } else { 12 }
    }
    pub fn summary(&self) -> String {
        if self.error && self.message.starts_with("认证未完成") {
            return "认证未完成，请在连接页查看原因。".into();
        }
        if self.error
            && (self.message.starts_with("连接") || self.message.starts_with("设备正在配对"))
        {
            return "连接未完成，请在连接页查看原因。".into();
        }
        let line = self.message.lines().next().unwrap_or_default();
        let mut text: String = line.chars().take(48).collect();
        if line.chars().count() > 48 {
            text.push('…');
        }
        text
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_and_result_have_distinct_lifetimes() {
        let mut f = Feedback {
            message: "正在读取".into(),
            busy: true,
            error: false,
            at: 100,
        };
        assert!(f.visible(150));
        f.busy = false;
        assert!(f.visible(111));
        assert!(!f.visible(112));
    }
    #[test]
    fn long_errors_do_not_expand_the_status_bar() {
        let f = Feedback::new(
            "认证未完成：很长的协议错误，详细信息只在连接页显示。".repeat(20),
            false,
            true,
        );
        assert_eq!(f.summary(), "认证未完成，请在连接页查看原因。");
        assert!(!f.summary().contains('\n'));
        assert!(
            Feedback::new("很长的普通错误".repeat(100), false, true)
                .summary()
                .chars()
                .count()
                <= 49
        );
    }
}
