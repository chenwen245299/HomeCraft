//! System-owned login item registration. Merely starting HomeCraft never registers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LoginItemStatus {
    #[default]
    Disabled,
    Enabled,
    RequiresApproval,
    Unavailable,
}
impl LoginItemStatus {
    pub fn selected(self) -> bool {
        matches!(self, Self::Enabled | Self::RequiresApproval)
    }
}

pub fn login_item_status() -> LoginItemStatus {
    #[cfg(target_os = "macos")]
    {
        imp::status()
    }
    #[cfg(not(target_os = "macos"))]
    {
        LoginItemStatus::Unavailable
    }
}
pub fn set_login_item(enabled: bool) -> Result<LoginItemStatus, String> {
    #[cfg(target_os = "macos")]
    {
        imp::set(enabled)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
        Err("此系统不支持登录自启动".into())
    }
}
pub fn open_login_items() {
    #[cfg(target_os = "macos")]
    {
        imp::open_settings();
    }
}
pub fn launched_at_login() -> bool {
    #[cfg(target_os = "macos")]
    {
        imp::launched_at_login()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod imp {
    use super::LoginItemStatus;
    use objc2::{
        msg_send,
        runtime::{AnyClass, AnyObject, Bool},
    };

    // The framework exists on macOS 12; the class is looked up at runtime for macOS 13+.
    #[link(name = "ServiceManagement", kind = "framework")]
    unsafe extern "C" {}

    fn service() -> Option<*mut AnyObject> {
        if !std::env::current_exe()
            .ok()?
            .ancestors()
            .any(|p| p.extension().is_some_and(|e| e == "app"))
        {
            return None;
        }
        let class = AnyClass::get("SMAppService")?;
        // SAFETY: SMAppService's mainAppService is a process-lifetime system object.
        Some(unsafe { msg_send![class, mainAppService] })
    }
    pub fn status() -> LoginItemStatus {
        let Some(service) = service() else {
            return LoginItemStatus::Unavailable;
        };
        // SAFETY: the dynamically available class implements status (NSInteger).
        match unsafe { msg_send![service, status] } {
            0isize => LoginItemStatus::Disabled,
            1 => LoginItemStatus::Enabled,
            2 => LoginItemStatus::RequiresApproval,
            // No job exists yet (or an old moved bundle's registration was removed).
            // Keep the control usable so register() can create the main-app login item.
            3 => LoginItemStatus::Disabled,
            _ => LoginItemStatus::Unavailable,
        }
    }
    pub fn set(enabled: bool) -> Result<LoginItemStatus, String> {
        let service = service().ok_or("请在 macOS 13 或更新版本中从 HomeCraft.app 设置自启动")?;
        let old = status();
        if (enabled && old == LoginItemStatus::Enabled)
            || (!enabled && old == LoginItemStatus::Disabled)
        {
            return Ok(old);
        }
        // SAFETY: system-owned main-app service; null NSError output is explicitly supported.
        let success: Bool = unsafe {
            if enabled {
                msg_send![service, registerAndReturnError: std::ptr::null_mut::<*mut AnyObject>()]
            } else {
                msg_send![service, unregisterAndReturnError: std::ptr::null_mut::<*mut AnyObject>()]
            }
        };
        let actual = status();
        if (success.as_bool() && enabled == actual.selected())
            || (enabled && actual.selected())
            || (!enabled && actual == LoginItemStatus::Disabled)
        {
            Ok(actual)
        } else {
            Err("自启动未保存，请在系统设置的“登录项”中检查 HomeCraft".into())
        }
    }
    pub fn open_settings() {
        if let Some(class) = AnyClass::get("SMAppService") {
            // SAFETY: invoked only in response to the settings button.
            unsafe {
                let _: () = msg_send![class, openSystemSettingsLoginItems];
            }
        }
    }
    pub fn launched_at_login() -> bool {
        // SAFETY: Foundation's event objects are inspected on the app's main thread.
        unsafe {
            let manager = objc2_foundation::NSAppleEventManager::sharedAppleEventManager();
            let Some(event) = manager.currentAppleEvent() else {
                return false;
            };
            let property: *mut AnyObject =
                msg_send![&*event, paramDescriptorForKeyword: u32::from_be_bytes(*b"prdt")];
            let login: *mut AnyObject =
                msg_send![&*event, paramDescriptorForKeyword: u32::from_be_bytes(*b"lgit")];
            !login.is_null()
                || (!property.is_null() && {
                    let code: u32 = msg_send![property, enumCodeValue];
                    code == u32::from_be_bytes(*b"lgit")
                })
        }
    }
}
