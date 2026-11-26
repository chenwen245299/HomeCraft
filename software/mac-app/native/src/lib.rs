//! The small AppKit boundary: real window buttons and a stable opaque window.
//! Called only for this app's borrowed eframe window, never other applications.
use raw_window_handle::HasWindowHandle;
mod login_item;
pub use login_item::{
    LoginItemStatus, launched_at_login, login_item_status, open_login_items, set_login_item,
};

/// User-mediated document folder selection; no automatic cloud-container discovery.
pub fn choose_sync_directory() -> Option<std::path::PathBuf> {
    imp::choose_directory()
}
pub fn is_icloud_directory(path: &std::path::Path) -> bool {
    imp::is_icloud(path)
}
/// Run off the main thread. Coordinate with the system's document sync service.
pub fn coordinated<T>(
    path: &std::path::Path,
    write: bool,
    operation: impl FnOnce(&std::path::Path) -> std::io::Result<T>,
) -> std::io::Result<T> {
    imp::coordinated(path, write, operation)
}

/// Native executable picker. Returns only the user's selected local path; no execution.
pub fn choose_executable(title: &str) -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        imp::choose_executable(title)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = title;
        None
    }
}

/// Updates window chrome without scheduling redraws. Returns whether it is configured.
pub fn update(window: &impl HasWindowHandle, installed: bool, fullscreen: bool) -> bool {
    #[cfg(target_os = "macos")]
    {
        imp::update(window, installed, fullscreen)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, fullscreen);
        installed
    }
}

/// Inspect only our borrowed window for native rendering diagnostics.
pub fn is_opaque(window: &impl HasWindowHandle) -> bool {
    #[cfg(target_os = "macos")]
    {
        imp::is_opaque(window)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        true
    }
}

/// Keep the Dock entry while a window is open (including minimized windows).
/// Hiding the closed main window changes only presentation, never the backend.
pub fn set_dock_visible(visible: bool) -> bool {
    #[cfg(target_os = "macos")]
    {
        imp::set_dock_visible(visible)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = visible;
        true
    }
}

/// Read the system's actual presentation policy, for local lifecycle diagnostics.
pub fn dock_visible() -> bool {
    #[cfg(target_os = "macos")]
    {
        imp::dock_visible()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod imp {
    use objc2_app_kit::{
        NSAppearance, NSAppearanceCustomization, NSAppearanceNameDarkAqua, NSApplication,
        NSApplicationActivationPolicy, NSColor, NSView, NSWindowButton,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub fn set_dock_visible(visible: bool) -> bool {
        let Some(main) = MainThreadMarker::new() else {
            return false;
        };
        NSApplication::sharedApplication(main).setActivationPolicy(if visible {
            NSApplicationActivationPolicy::Regular
        } else {
            NSApplicationActivationPolicy::Accessory
        })
    }
    pub fn dock_visible() -> bool {
        let Some(main) = MainThreadMarker::new() else {
            return false;
        };
        // SAFETY: only inspect this process's NSApplication on AppKit's main thread.
        unsafe {
            NSApplication::sharedApplication(main).activationPolicy()
                == NSApplicationActivationPolicy::Regular
        }
    }

    pub fn choose_directory() -> Option<std::path::PathBuf> {
        let main = MainThreadMarker::new()?;
        // SAFETY: main-thread-only panel. Only the chosen URL's owned path escapes.
        unsafe {
            let panel = objc2_app_kit::NSOpenPanel::openPanel(main);
            if let Some(a) = NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) {
                panel.setAppearance(Some(&a));
            }
            panel.setCanChooseFiles(false);
            panel.setCanChooseDirectories(true);
            panel.setCanCreateDirectories(true);
            panel.setAllowsMultipleSelection(false);
            panel.setTitle(Some(&objc2_foundation::NSString::from_str(
                "选择 iCloud Drive 同步文件夹",
            )));
            panel.setMessage(Some(&objc2_foundation::NSString::from_str(
                "在 iCloud Drive 中新建或选择 HomeCraft 文件夹；其他 Mac 选择同一个文件夹。",
            )));
            panel.setPrompt(Some(&objc2_foundation::NSString::from_str("使用此文件夹")));
            if panel.runModal() != 1 {
                return None;
            }
            let url = panel.URL()?;
            Some(std::path::PathBuf::from(url.path()?.to_string()))
        }
    }
    pub fn is_icloud(path: &std::path::Path) -> bool {
        // SAFETY: Foundation URL/manager own their values and accept an existing path.
        unsafe {
            let url = objc2_foundation::NSURL::fileURLWithPath(
                &objc2_foundation::NSString::from_str(&path.to_string_lossy()),
            );
            objc2_foundation::NSFileManager::defaultManager().isUbiquitousItemAtURL(&url)
        }
    }
    pub fn coordinated<T>(
        path: &std::path::Path,
        write: bool,
        operation: impl FnOnce(&std::path::Path) -> std::io::Result<T>,
    ) -> std::io::Result<T> {
        use objc2_foundation::{
            NSFileCoordinator, NSFileCoordinatorReadingOptions as R,
            NSFileCoordinatorWritingOptions as W, NSString, NSURL,
        };
        use std::{cell::RefCell, ptr::NonNull};
        let operation = RefCell::new(Some(operation));
        let result = RefCell::new(None);
        // SAFETY: coordinator invokes its nonescaping accessor synchronously; URL is
        // valid only within the callback and copied to an owned path before use.
        unsafe {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
            let manager = objc2_foundation::NSFileManager::defaultManager();
            if !write {
                let _ = manager.startDownloadingUbiquitousItemAtURL_error(&url);
            }
            let coordinator = NSFileCoordinator::new();
            let block = block2::RcBlock::new(|url: NonNull<NSURL>| {
                if let Some(operation) = operation.borrow_mut().take() {
                    let value = url
                        .as_ref()
                        .path()
                        .map(|s| std::path::PathBuf::from(s.to_string()));
                    *result.borrow_mut() = Some(
                        value
                            .ok_or_else(|| std::io::Error::other("同步文件不可用"))
                            .and_then(|p| operation(&p)),
                    );
                }
            });
            let mut error = None;
            if write {
                coordinator.coordinateWritingItemAtURL_options_error_byAccessor(
                    &url,
                    W::NSFileCoordinatorWritingForReplacing,
                    Some(&mut error),
                    &block,
                );
            } else {
                coordinator.coordinateReadingItemAtURL_options_error_byAccessor(
                    &url,
                    R::empty(),
                    Some(&mut error),
                    &block,
                );
            }
        }
        result
            .into_inner()
            .unwrap_or_else(|| Err(std::io::Error::other("iCloud 文件协调未完成，请稍后重试")))
    }

    pub fn choose_executable(title: &str) -> Option<std::path::PathBuf> {
        let main = MainThreadMarker::new()?;
        // SAFETY: this native modal panel is created, run and released on AppKit's main
        // thread. Only the selected URL is copied to Rust; no borrowed ObjC value escapes.
        unsafe {
            let panel = objc2_app_kit::NSOpenPanel::openPanel(main);
            if let Some(appearance) = NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) {
                panel.setAppearance(Some(&appearance));
            }
            panel.setCanChooseFiles(true);
            panel.setCanChooseDirectories(false);
            panel.setAllowsMultipleSelection(false);
            panel.setShowsHiddenFiles(true);
            panel.setTitle(Some(&objc2_foundation::NSString::from_str(title)));
            panel.setPrompt(Some(&objc2_foundation::NSString::from_str("选择")));
            if panel.runModal() != 1 {
                return None;
            }
            let url = panel.URL()?;
            if !url.isFileURL() {
                return None;
            }
            Some(std::path::PathBuf::from(url.path()?.to_string()))
        }
    }

    pub fn update(window: &impl HasWindowHandle, installed: bool, fullscreen: bool) -> bool {
        let Some(_main) = MainThreadMarker::new() else {
            return installed;
        };
        let Ok(handle) = window.window_handle() else {
            return installed;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return installed;
        };
        // SAFETY: the borrowed eframe window owns this live NSView. AppKit is main-thread-only,
        // checked above; no Objective-C pointer is retained past this call.
        let content = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let Some(win) = content.window() else {
            return installed;
        };
        if !fullscreen {
            for (index, kind) in [
                NSWindowButton::NSWindowCloseButton,
                NSWindowButton::NSWindowMiniaturizeButton,
                NSWindowButton::NSWindowZoomButton,
            ]
            .iter()
            .copied()
            .enumerate()
            {
                if let Some(button) = win.standardWindowButton(kind) {
                    let current = button.frame();
                    // Leave the titlebar's native hit testing and fullscreen behaviour intact.
                    let desired = NSPoint::new(
                        20.0 + index as f64 * 20.0,
                        (52.0 - current.size.height) / 2.0,
                    );
                    // SAFETY: move only a standard button belonging to our live window on main.
                    unsafe {
                        if let Some(parent) = button.superview() {
                            if let Some(container) = parent.superview() {
                                let mut frame = container.frame();
                                let top = win.frame().size.height - 52.0;
                                if (frame.size.height - 52.0).abs() > 0.1
                                    || (frame.origin.y - top).abs() > 0.1
                                {
                                    frame.size.height = 52.0;
                                    frame.origin.y = top;
                                    container.setFrame(frame);
                                }
                            }
                            let mut bounds = parent.frame();
                            if (bounds.size.height - 52.0).abs() > 0.1 {
                                bounds.size.height = 52.0;
                                parent.setFrame(bounds);
                            }
                        }
                        if (current.origin.x - desired.x).abs() > 0.1
                            || (current.origin.y - desired.y).abs() > 0.1
                        {
                            button.setFrameOrigin(desired);
                        }
                    }
                }
            }
        }
        if installed {
            return true;
        }
        // The App paints an opaque wallpaper and its own frosted surfaces. Match the
        // native window to the GPU surface instead of compositing another animated
        // NSVisualEffectView behind a transparent Metal layer on every presentation.
        // SAFETY: configure only the borrowed live window on AppKit's main thread.
        unsafe {
            win.setOpaque(true);
            win.setBackgroundColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(
                24. / 255.,
                39. / 255.,
                48. / 255.,
                1.,
            )));
            if let Some(appearance) = NSAppearance::appearanceNamed(NSAppearanceNameDarkAqua) {
                win.setAppearance(Some(&appearance));
            }
        }
        true
    }
    pub fn is_opaque(window: &impl HasWindowHandle) -> bool {
        if MainThreadMarker::new().is_none() {
            return false;
        }
        let Ok(handle) = window.window_handle() else {
            return false;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return false;
        };
        // SAFETY: inspect only this borrowed live NSView on the main thread.
        let content = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        content
            .window()
            .is_some_and(|win| unsafe { win.isOpaque() })
    }
}

/// Brings our app forward when launched or reopened from its menu-bar icon.
/// Winit's Focus alone does not activate a background/accessory macOS application.
#[allow(deprecated)] // activateIgnoringOtherApps is available on our macOS 12 minimum.
pub fn activate() {
    #[cfg(target_os = "macos")]
    {
        if let Some(main) = objc2_foundation::MainThreadMarker::new() {
            set_dock_visible(true);
            objc2_app_kit::NSApplication::sharedApplication(main).activateIgnoringOtherApps(true);
        }
    }
}
