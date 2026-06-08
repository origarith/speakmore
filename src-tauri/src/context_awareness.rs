use chrono::Utc;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::time::Instant;

pub const CONTEXT_TEXT_MAX_CHARS: usize = 200_000;
const TERMINAL_CONTEXT_MAX_CHARS: usize = 2_000;
const TERMINAL_CONTEXT_MAX_LINES: usize = 30;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ContextProbeStatus {
    Success,
    PermissionDenied,
    NoFocusedElement,
    NotTextInput,
    BlockedSecureField,
    AttributeUnavailable,
    UnsupportedPlatform,
    Error,
}

impl ContextProbeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::PermissionDenied => "permission_denied",
            Self::NoFocusedElement => "no_focused_element",
            Self::NotTextInput => "not_text_input",
            Self::BlockedSecureField => "blocked_secure_field",
            Self::AttributeUnavailable => "attribute_unavailable",
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::Error => "error",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "success" => Self::Success,
            "permission_denied" => Self::PermissionDenied,
            "no_focused_element" => Self::NoFocusedElement,
            "not_text_input" => Self::NotTextInput,
            "blocked_secure_field" => Self::BlockedSecureField,
            "attribute_unavailable" => Self::AttributeUnavailable,
            "unsupported_platform" => Self::UnsupportedPlatform,
            _ => Self::Error,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Type)]
#[serde(rename_all = "snake_case")]
pub enum ContextProbeConfidence {
    High,
    Medium,
    Low,
    None,
}

impl ContextProbeConfidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::None => "none",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "high" => Self::High,
            "medium" => Self::Medium,
            "low" => Self::Low,
            _ => Self::None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct FocusedTextContext {
    pub before_text: String,
    pub selected_text: String,
    pub after_text: String,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NewContextProbeRun {
    pub history_entry_id: Option<i64>,
    pub captured_at: i64,
    pub source: String,
    pub status: ContextProbeStatus,
    pub confidence: ContextProbeConfidence,
    pub latency_ms: i64,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
    pub pid: Option<i64>,
    pub window_title: Option<String>,
    pub element_role: Option<String>,
    pub element_subrole: Option<String>,
    pub is_secure: bool,
    pub value_text: Option<String>,
    pub before_text: Option<String>,
    pub selected_text: Option<String>,
    pub after_text: Option<String>,
    pub selected_location_utf16: Option<i64>,
    pub selected_length_utf16: Option<i64>,
    pub number_of_characters: Option<i64>,
    pub available_attributes_json: Option<String>,
    pub failure_reason: Option<String>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct ContextProbeRun {
    pub id: i64,
    pub history_entry_id: Option<i64>,
    pub captured_at: i64,
    pub source: String,
    pub status: ContextProbeStatus,
    pub confidence: ContextProbeConfidence,
    pub latency_ms: i64,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
    pub pid: Option<i64>,
    pub window_title: Option<String>,
    pub element_role: Option<String>,
    pub element_subrole: Option<String>,
    pub is_secure: bool,
    pub value_text: Option<String>,
    pub before_text: Option<String>,
    pub selected_text: Option<String>,
    pub after_text: Option<String>,
    pub selected_location_utf16: Option<i64>,
    pub selected_length_utf16: Option<i64>,
    pub number_of_characters: Option<i64>,
    pub available_attributes_json: Option<String>,
    pub failure_reason: Option<String>,
    pub truncated: bool,
}

impl ContextProbeRun {
    pub fn focused_text_context(&self) -> Option<FocusedTextContext> {
        if self.status != ContextProbeStatus::Success || self.is_secure {
            return None;
        }

        Some(FocusedTextContext {
            before_text: self.before_text.clone().unwrap_or_default(),
            selected_text: self.selected_text.clone().unwrap_or_default(),
            after_text: self.after_text.clone().unwrap_or_default(),
            app_name: self.app_name.clone(),
            bundle_id: self.bundle_id.clone(),
            window_title: self.window_title.clone(),
        })
    }
}

impl NewContextProbeRun {
    fn base(source: String, started_at: Instant) -> Self {
        Self {
            history_entry_id: None,
            captured_at: Utc::now().timestamp(),
            source,
            status: ContextProbeStatus::Error,
            confidence: ContextProbeConfidence::None,
            latency_ms: elapsed_ms(started_at),
            app_name: None,
            bundle_id: None,
            pid: None,
            window_title: None,
            element_role: None,
            element_subrole: None,
            is_secure: false,
            value_text: None,
            before_text: None,
            selected_text: None,
            after_text: None,
            selected_location_utf16: None,
            selected_length_utf16: None,
            number_of_characters: None,
            available_attributes_json: None,
            failure_reason: None,
            truncated: false,
        }
    }

    pub fn focused_text_context(&self) -> Option<FocusedTextContext> {
        if self.status != ContextProbeStatus::Success || self.is_secure {
            return None;
        }

        Some(FocusedTextContext {
            before_text: self.before_text.clone().unwrap_or_default(),
            selected_text: self.selected_text.clone().unwrap_or_default(),
            after_text: self.after_text.clone().unwrap_or_default(),
            app_name: self.app_name.clone(),
            bundle_id: self.bundle_id.clone(),
            window_title: self.window_title.clone(),
        })
    }

    fn with_status(
        source: String,
        started_at: Instant,
        status: ContextProbeStatus,
        failure_reason: Option<String>,
    ) -> Self {
        let mut run = Self::base(source, started_at);
        run.status = status;
        run.failure_reason = failure_reason;
        run
    }
}

pub fn capture_focused_context(source: String) -> NewContextProbeRun {
    let started_at = Instant::now();
    let source = normalize_source(source);

    #[cfg(target_os = "macos")]
    {
        match platform::capture(&source, started_at) {
            Ok(run) => run,
            Err(error) => NewContextProbeRun::with_status(
                source,
                started_at,
                ContextProbeStatus::Error,
                Some(error),
            ),
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        NewContextProbeRun::with_status(
            source,
            started_at,
            ContextProbeStatus::UnsupportedPlatform,
            Some("Context probing is only supported on macOS".to_string()),
        )
    }
}

fn elapsed_ms(started_at: Instant) -> i64 {
    started_at.elapsed().as_millis().min(i64::MAX as u128) as i64
}

fn normalize_source(source: String) -> String {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        "manual".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

pub(crate) fn cap_text_field(value: String) -> (String, bool) {
    if value.chars().count() <= CONTEXT_TEXT_MAX_CHARS {
        return (value, false);
    }

    (value.chars().take(CONTEXT_TEXT_MAX_CHARS).collect(), true)
}

pub(crate) fn split_by_utf16_range(
    text: &str,
    location: i64,
    length: i64,
) -> Result<(String, String, String), String> {
    if location < 0 || length < 0 {
        return Err("Selected text range contains a negative offset".to_string());
    }

    let start_units = location as usize;
    let end_units = start_units
        .checked_add(length as usize)
        .ok_or_else(|| "Selected text range overflows".to_string())?;
    let start = byte_index_for_utf16_offset(text, start_units).ok_or_else(|| {
        "Selected text range starts inside a character or beyond text".to_string()
    })?;
    let end = byte_index_for_utf16_offset(text, end_units)
        .ok_or_else(|| "Selected text range ends inside a character or beyond text".to_string())?;

    if start > end {
        return Err("Selected text range is reversed".to_string());
    }

    Ok((
        text[..start].to_string(),
        text[start..end].to_string(),
        text[end..].to_string(),
    ))
}

fn byte_index_for_utf16_offset(text: &str, target_units: usize) -> Option<usize> {
    let mut units = 0usize;

    for (byte_index, ch) in text.char_indices() {
        if units == target_units {
            return Some(byte_index);
        }

        units += ch.len_utf16();
        if units > target_units {
            return None;
        }
    }

    if units == target_units {
        Some(text.len())
    } else {
        None
    }
}

pub(crate) fn looks_like_secure_field(values: &[Option<String>]) -> bool {
    let haystack = values
        .iter()
        .filter_map(|value| value.as_deref())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    ["secure", "password", "passcode", "pin", "secret"]
        .iter()
        .any(|needle| haystack.contains(needle))
}

pub(crate) fn looks_like_terminal_app(app_name: Option<&str>, bundle_id: Option<&str>) -> bool {
    let haystack = [app_name, bundle_id]
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    [
        "iterm",
        "com.googlecode.iterm2",
        "terminal",
        "com.apple.terminal",
        "warp",
        "dev.warp",
        "wezterm",
        "alacritty",
        "ghostty",
        "kitty",
        "hyper",
        "tabby",
    ]
    .iter()
    .any(|needle| haystack.contains(needle))
}

pub(crate) fn looks_like_iterm2_app(app_name: Option<&str>, bundle_id: Option<&str>) -> bool {
    let haystack = [app_name, bundle_id]
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    haystack.contains("iterm") || haystack.contains("com.googlecode.iterm2")
}

pub(crate) fn terminal_tail_context(value: &str) -> (String, bool) {
    let lines = value.lines().collect::<Vec<_>>();
    let line_truncated = lines.len() > TERMINAL_CONTEXT_MAX_LINES;
    let line_tail = if line_truncated {
        lines[lines.len() - TERMINAL_CONTEXT_MAX_LINES..].join("\n")
    } else {
        value.to_string()
    };

    let char_count = line_tail.chars().count();
    if char_count <= TERMINAL_CONTEXT_MAX_CHARS {
        return (line_tail, line_truncated);
    }

    (
        line_tail
            .chars()
            .skip(char_count - TERMINAL_CONTEXT_MAX_CHARS)
            .collect(),
        true,
    )
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{
        cap_text_field, elapsed_ms, looks_like_iterm2_app, looks_like_secure_field,
        looks_like_terminal_app, split_by_utf16_range, terminal_tail_context,
        ContextProbeConfidence, ContextProbeStatus, NewContextProbeRun,
    };
    use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
    use core_foundation_sys::attributed_string::{
        CFAttributedStringGetString, CFAttributedStringGetTypeID, CFAttributedStringRef,
    };
    use core_foundation_sys::base::{
        kCFAllocatorDefault, CFGetTypeID, CFHash, CFRange, CFRelease, CFRetain, CFTypeID, CFTypeRef,
    };
    use core_foundation_sys::number::{
        kCFBooleanTrue, kCFNumberSInt64Type, CFBooleanGetTypeID, CFBooleanGetValue, CFBooleanRef,
        CFNumberGetTypeID, CFNumberGetValue, CFNumberRef,
    };
    use core_foundation_sys::string::{
        kCFStringEncodingUTF8, CFStringCreateWithCString, CFStringGetCString,
        CFStringGetMaximumSizeForEncoding, CFStringGetTypeID, CFStringRef,
    };
    use objc::runtime::{Class, Object, Sel};
    use objc::Message;
    use std::collections::{HashSet, VecDeque};
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_float, c_int, c_void};
    use std::process::{Command, Stdio};
    use std::ptr;
    use std::thread;
    use std::time::{Duration, Instant};

    type AXUIElementRef = *const c_void;
    type AXTextMarkerRef = *const c_void;
    type AXTextMarkerRangeRef = *const c_void;
    type AXValueRef = *const c_void;
    type AXError = c_int;

    const AX_ERROR_SUCCESS: AXError = 0;
    const AX_ERROR_FAILURE: AXError = -25200;
    const AX_ERROR_ILLEGAL_ARGUMENT: AXError = -25201;
    const AX_ERROR_INVALID_UI_ELEMENT: AXError = -25202;
    const AX_ERROR_INVALID_UI_ELEMENT_OBSERVER: AXError = -25203;
    const AX_ERROR_CANNOT_COMPLETE: AXError = -25204;
    const AX_ERROR_ATTRIBUTE_UNSUPPORTED: AXError = -25205;
    const AX_ERROR_ACTION_UNSUPPORTED: AXError = -25206;
    const AX_ERROR_NOTIFICATION_UNSUPPORTED: AXError = -25207;
    const AX_ERROR_NOT_IMPLEMENTED: AXError = -25208;
    const AX_ERROR_NOTIFICATION_ALREADY_REGISTERED: AXError = -25209;
    const AX_ERROR_NOTIFICATION_NOT_REGISTERED: AXError = -25210;
    const AX_ERROR_API_DISABLED: AXError = -25211;
    const AX_ERROR_NO_VALUE: AXError = -25212;
    const AX_ERROR_PARAMETERIZED_ATTRIBUTE_UNSUPPORTED: AXError = -25213;
    const AX_ERROR_NOT_ENOUGH_PRECISION: AXError = -25214;
    const AX_VALUE_CF_RANGE_TYPE: c_int = 4;
    const AX_TIMEOUT_SECONDS: c_float = 0.25;
    const CHILD_RELATION_ATTRIBUTES: &[&str] = &[
        "AXChildren",
        "AXVisibleChildren",
        "AXContents",
        "AXNextContents",
        "AXPreviousContents",
        "AXSelectedChildren",
        "AXLinkedUIElements",
        "AXLabelUIElements",
        "AXRows",
        "AXVisibleRows",
        "AXSelectedRows",
        "AXColumns",
        "AXVisibleColumns",
        "AXSelectedColumns",
        "AXVisibleCells",
        "AXSelectedCells",
        "AXEditableAncestor",
        "AXHighestEditableAncestor",
        "AXFocusableAncestor",
    ];
    const TEXT_VALUE_ATTRIBUTES: &[&str] = &[
        "AXValue",
        "AXText",
        "AXVisibleText",
        "AXSelectedText",
        "AXLabelValue",
        "AXValueDescription",
    ];

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> u8;
        fn AXUIElementCreateApplication(pid: c_int) -> AXUIElementRef;
        fn AXUIElementCreateSystemWide() -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
        fn AXUIElementCopyAttributeNames(
            element: AXUIElementRef,
            names: *mut CFArrayRef,
        ) -> AXError;
        fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut c_int) -> AXError;
        fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout: c_float) -> AXError;
        fn AXUIElementCopyParameterizedAttributeNames(
            element: AXUIElementRef,
            names: *mut CFArrayRef,
        ) -> AXError;
        fn AXUIElementCopyParameterizedAttributeValue(
            element: AXUIElementRef,
            parameterized_attribute: CFStringRef,
            parameter: CFTypeRef,
            result: *mut CFTypeRef,
        ) -> AXError;
        fn AXUIElementSetAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: CFTypeRef,
        ) -> AXError;
        fn AXUIElementGetTypeID() -> CFTypeID;
        fn AXTextMarkerGetTypeID() -> CFTypeID;
        fn AXTextMarkerRangeGetTypeID() -> CFTypeID;
        fn AXTextMarkerRangeCreate(
            allocator: CFTypeRef,
            start_marker: AXTextMarkerRef,
            end_marker: AXTextMarkerRef,
        ) -> AXTextMarkerRangeRef;
        fn AXTextMarkerRangeCopyStartMarker(
            text_marker_range: AXTextMarkerRangeRef,
        ) -> AXTextMarkerRef;
        fn AXTextMarkerRangeCopyEndMarker(
            text_marker_range: AXTextMarkerRangeRef,
        ) -> AXTextMarkerRef;
        fn AXValueGetType(value: AXValueRef) -> c_int;
        fn AXValueGetTypeID() -> CFTypeID;
        fn AXValueGetValue(value: AXValueRef, value_type: c_int, out_value: *mut c_void) -> u8;
        fn AXValueCreate(value_type: c_int, value: *const c_void) -> AXValueRef;
    }

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}

    pub fn capture(source: &str, started_at: Instant) -> Result<NewContextProbeRun, String> {
        if unsafe { AXIsProcessTrusted() } == 0 {
            return Ok(NewContextProbeRun::with_status(
                source.to_string(),
                started_at,
                ContextProbeStatus::PermissionDenied,
                Some("Accessibility permission is not granted".to_string()),
            ));
        }

        let mut run = NewContextProbeRun::base(source.to_string(), started_at);
        let system = unsafe { AXUIElementCreateSystemWide() };
        if system.is_null() {
            return Ok(NewContextProbeRun::with_status(
                source.to_string(),
                started_at,
                ContextProbeStatus::Error,
                Some("AXUIElementCreateSystemWide returned null".to_string()),
            ));
        }

        unsafe {
            let _ = AXUIElementSetMessagingTimeout(system, AX_TIMEOUT_SECONDS);
        }

        let mut focus_lookup_failures = Vec::new();
        let focused_app = match copy_attr_element(system, "AXFocusedApplication") {
            Ok(Some(app)) => Some(app),
            Ok(None) => {
                focus_lookup_failures.push("system focused app: no value".to_string());
                None
            }
            Err(error) => {
                focus_lookup_failures.push(format!(
                    "system focused app: {}",
                    ax_error_description(error)
                ));
                None
            }
        };
        let frontmost_app = frontmost_application();
        if frontmost_app.is_none() {
            focus_lookup_failures.push("frontmost app: unavailable from NSWorkspace".to_string());
        }
        let mut focused_window = None;
        if let Some(app) = focused_app {
            unsafe {
                let _ = AXUIElementSetMessagingTimeout(app, AX_TIMEOUT_SECONDS);
            }
            enable_rich_accessibility(app);
            let _ = copy_attr_string(app, "AXRole");
            run.app_name = copy_attr_string(app, "AXTitle").ok().flatten();
            run.bundle_id = copy_attr_string(app, "AXIdentifier").ok().flatten();
            run.pid = ax_pid(app).map(i64::from);

            if let Ok(Some(window)) = copy_attr_element(app, "AXFocusedWindow") {
                unsafe {
                    let _ = AXUIElementSetMessagingTimeout(window, AX_TIMEOUT_SECONDS);
                }
                enable_rich_accessibility(window);
                let _ = copy_attr_string(window, "AXRole");
                run.window_title = copy_attr_string(window, "AXTitle").ok().flatten();
                focused_window = Some(window);
            }
        }
        if let Some(app) = &frontmost_app {
            unsafe {
                let _ = AXUIElementSetMessagingTimeout(app.element, AX_TIMEOUT_SECONDS);
            }
            enable_rich_accessibility(app.element);
            run.app_name = run.app_name.clone().or_else(|| app.app_name.clone());
            run.bundle_id = run.bundle_id.clone().or_else(|| app.bundle_id.clone());
            run.pid = run.pid.or(Some(i64::from(app.pid)));
        }

        let mut frontmost_window = None;
        if let Some(app) = &frontmost_app {
            if let Ok(Some(window)) = copy_attr_element(app.element, "AXFocusedWindow") {
                unsafe {
                    let _ = AXUIElementSetMessagingTimeout(window, AX_TIMEOUT_SECONDS);
                }
                enable_rich_accessibility(window);
                let _ = copy_attr_string(window, "AXRole");
                if run.window_title.is_none() {
                    run.window_title = copy_attr_string(window, "AXTitle").ok().flatten();
                }
                frontmost_window = Some(window);
            }
        }

        let focused_element = match find_focused_element(
            system,
            focused_app,
            frontmost_app.as_ref().map(|app| app.element),
            focused_window,
            frontmost_window,
            focus_lookup_failures,
        ) {
            Ok(element) => element,
            Err(failures) => {
                release_optional(frontmost_window);
                release_optional(focused_window);
                release_optional(frontmost_app.as_ref().map(|app| app.element));
                release_optional(focused_app);
                unsafe { CFRelease(system as CFTypeRef) };
                run.status = ContextProbeStatus::NoFocusedElement;
                run.failure_reason = Some(format!(
                    "No focused accessibility element was returned. Tried: {}",
                    failures.join("; ")
                ));
                run.latency_ms = elapsed_ms(started_at);
                return Ok(run);
            }
        };
        let focused_element_source = focused_element.source.clone();
        let focused_element = focused_element.element;

        unsafe {
            let _ = AXUIElementSetMessagingTimeout(focused_element, AX_TIMEOUT_SECONDS);
        }

        run.element_role = copy_attr_string(focused_element, "AXRole").ok().flatten();
        run.element_subrole = copy_attr_string(focused_element, "AXSubrole")
            .ok()
            .flatten();
        let element_title = copy_attr_string(focused_element, "AXTitle").ok().flatten();
        let element_description = copy_attr_string(focused_element, "AXDescription")
            .ok()
            .flatten();
        let element_help = copy_attr_string(focused_element, "AXHelp").ok().flatten();
        let element_placeholder = copy_attr_string(focused_element, "AXPlaceholderValue")
            .ok()
            .flatten();
        let mut attributes = copy_attribute_names(focused_element).unwrap_or_default();
        if let Ok(parameterized_attributes) = copy_parameterized_attribute_names(focused_element) {
            attributes.extend(
                parameterized_attributes
                    .into_iter()
                    .map(|attribute| format!("parameterized:{attribute}")),
            );
        }
        run.available_attributes_json = serde_json::to_string(&attributes).ok();
        run.is_secure = looks_like_secure_field(&[
            run.element_role.clone(),
            run.element_subrole.clone(),
            element_title,
            element_description,
            element_help,
            element_placeholder,
        ]);

        if run.is_secure {
            release_optional(frontmost_window);
            release_optional(focused_window);
            release_optional(frontmost_app.as_ref().map(|app| app.element));
            release_optional(focused_app);
            unsafe {
                CFRelease(focused_element as CFTypeRef);
                CFRelease(system as CFTypeRef);
            }
            run.status = ContextProbeStatus::BlockedSecureField;
            run.confidence = ContextProbeConfidence::None;
            run.failure_reason =
                Some("Focused element appears to be a secure text field".to_string());
            run.latency_ms = elapsed_ms(started_at);
            return Ok(run);
        }

        run.number_of_characters = copy_attr_number(focused_element, "AXNumberOfCharacters")
            .ok()
            .flatten();

        let selected_text = copy_attr_string(focused_element, "AXSelectedText")
            .ok()
            .flatten();
        let selected_range = copy_attr_cf_range(focused_element, "AXSelectedTextRange")
            .ok()
            .flatten();
        let visible_range = copy_attr_cf_range(focused_element, "AXVisibleCharacterRange")
            .ok()
            .flatten();
        let role_is_text =
            is_text_role(run.element_role.as_deref(), run.element_subrole.as_deref());
        let raw_value = readable_text_for_interactive_element(focused_element)
            .or_else(|| copy_first_non_empty_attr_string(focused_element, &["AXValue"]));
        let marker_text = text_from_marker_apis(focused_element);
        let selected_range = marker_text
            .as_ref()
            .and_then(|text| text.range)
            .or(selected_range);
        let mut value_from_descendants = false;
        let value = if raw_value.as_deref().is_none_or(str::is_empty) {
            match marker_text
                .as_ref()
                .map(|text| text.text.clone())
                .or_else(|| collect_descendant_text(focused_element))
            {
                Some(text) if !text.is_empty() => {
                    value_from_descendants = true;
                    run.number_of_characters = run
                        .number_of_characters
                        .or(Some(text.encode_utf16().count() as i64));
                    Some(text)
                }
                _ => raw_value,
            }
        } else {
            raw_value
        };
        let terminal_like =
            looks_like_terminal_app(run.app_name.as_deref(), run.bundle_id.as_deref());

        match (value, selected_range) {
            (Some(value), Some(range))
                if value.is_empty()
                    && !role_is_text
                    && selected_text.as_deref().is_none_or(str::is_empty) =>
            {
                run.status = ContextProbeStatus::AttributeUnavailable;
                run.confidence = ContextProbeConfidence::Low;
                run.value_text = Some(value);
                run.selected_location_utf16 = Some(range.location);
                run.selected_length_utf16 = Some(range.length);
                run.failure_reason = Some(format!(
                    "Focused element from {focused_element_source} exposes an empty AXValue; descendant scan did not find readable text"
                ));
            }
            (Some(value), _) if terminal_like => {
                if value.trim().is_empty() && selected_text.as_deref().is_none_or(str::is_empty) {
                    run.status = ContextProbeStatus::AttributeUnavailable;
                    run.confidence = ContextProbeConfidence::Low;
                    run.value_text = Some(value);
                    run.failure_reason = Some(format!(
                        "Terminal text element from {focused_element_source} exposes an empty AXValue"
                    ));
                } else {
                    let terminal_context = terminal_context_text(
                        focused_element,
                        &value,
                        visible_range,
                        run.app_name.as_deref(),
                        run.bundle_id.as_deref(),
                    );
                    let selected = selected_text.unwrap_or_default();
                    let selected_len = selected.encode_utf16().count() as i64;
                    let cursor_location = terminal_context.text.encode_utf16().count() as i64;
                    let selection_hint = selected_range
                        .map(|range| format!("{}:{}", range.location, range.length))
                        .unwrap_or_else(|| "unavailable".to_string());

                    run.status = ContextProbeStatus::Success;
                    run.confidence = ContextProbeConfidence::Medium;
                    run.value_text = Some(terminal_context.text.clone());
                    run.before_text = Some(terminal_context.text);
                    run.selected_text = Some(selected);
                    run.after_text = Some(String::new());
                    run.selected_location_utf16 = Some(cursor_location);
                    run.selected_length_utf16 = Some(selected_len);
                    run.truncated = terminal_context.truncated;
                    run.failure_reason = Some(format!(
                        "Terminal context captured from {}; AX selected range {selection_hint} is treated as unreliable",
                        terminal_context.source
                    ));
                }
            }
            (Some(value), Some(range)) => {
                let mut effective_range = range;
                let mut range_degraded = false;
                let mut fallback_truncated = false;
                let split = split_by_utf16_range(&value, range.location, range.length);
                let (before, selected, after) = match split {
                    Ok(parts) => parts,
                    Err(error) => {
                        range_degraded = true;
                        let original_utf16_len = value.encode_utf16().count();
                        let (fallback_before, truncated) = if terminal_like {
                            terminal_tail_context(&value)
                        } else {
                            (value.clone(), false)
                        };
                        fallback_truncated = truncated;
                        effective_range = AxRange {
                            location: fallback_before.encode_utf16().count() as i64,
                            length: 0,
                        };

                        if terminal_like {
                            run.failure_reason = Some(format!(
                                "Terminal AX selected range {}:{} does not map to the returned text ({} UTF-16 units: {error}); using the terminal tail as before-context",
                                range.location, range.length, original_utf16_len
                            ));
                        } else if value_from_descendants {
                            run.failure_reason = Some(format!(
                                "Selected range from AX was incompatible with fallback text ({error}); using end-of-text cursor"
                            ));
                        } else {
                            run.failure_reason = Some(format!(
                                "Selected range from AX was incompatible with readable text ({error}); using end-of-text cursor"
                            ));
                        }

                        (fallback_before, String::new(), String::new())
                    }
                };

                let (value, value_truncated) = if terminal_like {
                    terminal_tail_context(&value)
                } else {
                    cap_text_field(value)
                };
                let (before, before_truncated) = if terminal_like && !range_degraded {
                    terminal_tail_context(&before)
                } else {
                    cap_text_field(before)
                };
                let (selected, selected_truncated) =
                    cap_text_field(selected_text.unwrap_or(selected));
                let (after, after_truncated) = cap_text_field(after);

                run.status = ContextProbeStatus::Success;
                run.confidence = if terminal_like || value_from_descendants || range_degraded {
                    ContextProbeConfidence::Medium
                } else {
                    ContextProbeConfidence::High
                };
                run.value_text = Some(value);
                run.before_text = Some(before);
                run.selected_text = Some(selected);
                run.after_text = Some(after);
                run.selected_location_utf16 = Some(effective_range.location);
                run.selected_length_utf16 = Some(effective_range.length);
                run.truncated = fallback_truncated
                    || value_truncated
                    || before_truncated
                    || selected_truncated
                    || after_truncated;
            }
            (Some(value), None) => {
                let (value, value_truncated) = if terminal_like {
                    terminal_tail_context(&value)
                } else {
                    cap_text_field(value.clone())
                };
                let (before, before_truncated) = if terminal_like {
                    (value.clone(), value_truncated)
                } else {
                    cap_text_field(value.clone())
                };
                run.status = ContextProbeStatus::Success;
                run.confidence = ContextProbeConfidence::Medium;
                run.value_text = Some(value);
                run.before_text = Some(before);
                run.selected_text = selected_text;
                run.after_text = Some(String::new());
                run.truncated = value_truncated || before_truncated;
                run.failure_reason = Some(
                    "Focused text value is readable, but selected range is unavailable".to_string(),
                );
            }
            (None, _) if role_is_text => {
                run.status = ContextProbeStatus::AttributeUnavailable;
                run.confidence = ContextProbeConfidence::Low;
                run.failure_reason = Some(format!(
                    "Focused text element from {focused_element_source} does not expose AXValue"
                ));
            }
            (None, _) => {
                run.status = ContextProbeStatus::NotTextInput;
                run.confidence = ContextProbeConfidence::Low;
                run.failure_reason = Some(format!(
                    "Focused element from {focused_element_source} is not a readable text input"
                ));
            }
        }

        release_optional(frontmost_window);
        release_optional(focused_window);
        release_optional(frontmost_app.as_ref().map(|app| app.element));
        release_optional(focused_app);
        unsafe {
            CFRelease(focused_element as CFTypeRef);
            CFRelease(system as CFTypeRef);
        }
        run.latency_ms = elapsed_ms(started_at);
        Ok(run)
    }

    #[derive(Clone, Copy)]
    struct AxRange {
        location: i64,
        length: i64,
    }

    struct FocusedElement {
        element: AXUIElementRef,
        source: String,
    }

    struct MarkerText {
        text: String,
        range: Option<AxRange>,
    }

    struct TerminalContextText {
        text: String,
        truncated: bool,
        source: String,
    }

    struct FrontmostApplication {
        element: AXUIElementRef,
        app_name: Option<String>,
        bundle_id: Option<String>,
        pid: c_int,
    }

    fn is_text_role(role: Option<&str>, subrole: Option<&str>) -> bool {
        [role, subrole].iter().flatten().any(|value| {
            matches!(
                *value,
                "AXTextField" | "AXTextArea" | "AXComboBox" | "AXSearchField"
            ) || value.to_ascii_lowercase().contains("text")
        })
    }

    fn ax_pid(element: AXUIElementRef) -> Option<c_int> {
        let mut pid = 0;
        let result = unsafe { AXUIElementGetPid(element, &mut pid) };
        if result == AX_ERROR_SUCCESS {
            Some(pid)
        } else {
            None
        }
    }

    fn release_optional(element: Option<AXUIElementRef>) {
        if let Some(element) = element {
            unsafe { CFRelease(element as CFTypeRef) };
        }
    }

    fn release_cf_if_not_null(value: CFTypeRef) {
        if !value.is_null() {
            unsafe { CFRelease(value) };
        }
    }

    fn retain_element(element: AXUIElementRef) -> AXUIElementRef {
        unsafe { CFRetain(element as CFTypeRef) as AXUIElementRef }
    }

    fn enable_rich_accessibility(element: AXUIElementRef) {
        let _ = set_attr_bool(element, "AXManualAccessibility", true);
        let _ = set_attr_bool(element, "AXEnhancedUserInterface", true);
    }

    fn frontmost_application() -> Option<FrontmostApplication> {
        unsafe {
            let workspace_class = Class::get("NSWorkspace")?;
            let workspace: *mut Object = workspace_class
                .send_message(Sel::register("sharedWorkspace"), ())
                .ok()?;
            if workspace.is_null() {
                return None;
            }

            let app: *mut Object = (&*workspace)
                .send_message(Sel::register("frontmostApplication"), ())
                .ok()?;
            if app.is_null() {
                return None;
            }

            let pid: c_int = (&*app)
                .send_message(Sel::register("processIdentifier"), ())
                .ok()?;
            if pid <= 0 {
                return None;
            }

            let element = AXUIElementCreateApplication(pid);
            if element.is_null() {
                return None;
            }

            Some(FrontmostApplication {
                element,
                app_name: ns_string_property(app, "localizedName"),
                bundle_id: ns_string_property(app, "bundleIdentifier"),
                pid,
            })
        }
    }

    unsafe fn ns_string_property(object: *mut Object, selector: &str) -> Option<String> {
        let ns_string: *mut Object = (&*object).send_message(Sel::register(selector), ()).ok()?;
        if ns_string.is_null() {
            return None;
        }

        let utf8: *const c_char = (&*ns_string)
            .send_message(Sel::register("UTF8String"), ())
            .ok()?;
        if utf8.is_null() {
            return None;
        }

        Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
    }

    enum ScanOutcome {
        Found {
            element: AXUIElementRef,
            visited: usize,
            reason: String,
        },
        NotFound {
            visited: usize,
            candidates: usize,
        },
    }

    fn scan_for_context_element(root: AXUIElementRef) -> ScanOutcome {
        const MAX_SCAN_NODES: usize = 1_200;
        const MAX_SCAN_DEPTH: usize = 18;

        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();
        let mut visited = 0usize;
        let mut candidates = 0usize;
        let mut best: Option<(AXUIElementRef, i32, String)> = None;

        queue.push_back((retain_element(root), 0usize));

        while let Some((element, depth)) = queue.pop_front() {
            let hash = unsafe { CFHash(element as CFTypeRef) };
            if !seen.insert(hash) {
                unsafe { CFRelease(element as CFTypeRef) };
                continue;
            }

            visited += 1;
            unsafe {
                let _ = AXUIElementSetMessagingTimeout(element, AX_TIMEOUT_SECONDS);
            }

            let role = copy_attr_string(element, "AXRole").ok().flatten();
            let subrole = copy_attr_string(element, "AXSubrole").ok().flatten();
            let focused = copy_attr_bool(element, "AXFocused").ok().flatten() == Some(true);
            let selected_range_available = copy_attr_cf_range(element, "AXSelectedTextRange")
                .ok()
                .flatten()
                .is_some();
            let selected_text_available =
                copy_first_non_empty_attr_string(element, &["AXSelectedText"]).is_some();
            let value_available = copy_first_non_empty_attr_string(element, TEXT_VALUE_ATTRIBUTES)
                .is_some()
                || text_from_marker_apis(element).is_some();
            let editable = copy_attr_bool(element, "AXIsEditable").ok().flatten() == Some(true);
            let role_is_text = is_text_role(role.as_deref(), subrole.as_deref());
            let role_name = role.as_deref().unwrap_or("unknown role");

            let mut score = 0;
            let mut reason = None;
            if focused
                && (role_is_text || selected_range_available || selected_text_available || editable)
            {
                score = 100;
                reason = Some(format!("focused text-like descendant {role_name}"));
            } else if focused && value_available {
                score = 90;
                reason = Some(format!("focused value descendant {role_name}"));
            } else if editable && value_available {
                score = 88;
                reason = Some(format!("editable value descendant {role_name}"));
            } else if editable {
                score = 85;
                reason = Some(format!("editable descendant {role_name}"));
            } else if selected_range_available {
                score = 80;
                reason = Some(format!("selection range descendant {role_name}"));
            } else if selected_text_available {
                score = 75;
                reason = Some(format!("selected text descendant {role_name}"));
            } else if role_is_text && (value_available || selected_text_available) {
                score = 70;
                reason = Some(format!("text descendant {role_name}"));
            } else if focused {
                score = 50;
                reason = Some(format!("focused descendant {role_name}"));
            }

            if let Some(reason) = reason {
                candidates += 1;
                if score >= 90 {
                    return ScanOutcome::Found {
                        element,
                        visited,
                        reason,
                    };
                }

                let should_replace = best
                    .as_ref()
                    .map(|(_, best_score, _)| score > *best_score)
                    .unwrap_or(true);
                if should_replace {
                    if let Some((old, _, _)) = best.take() {
                        unsafe { CFRelease(old as CFTypeRef) };
                    }
                    best = Some((retain_element(element), score, reason));
                }
            }

            if depth < MAX_SCAN_DEPTH && visited < MAX_SCAN_NODES {
                for attr in CHILD_RELATION_ATTRIBUTES {
                    if let Ok(children) = copy_attr_related_elements(element, attr) {
                        for child in children {
                            if queue.len() + visited >= MAX_SCAN_NODES {
                                unsafe { CFRelease(child as CFTypeRef) };
                                continue;
                            }
                            queue.push_back((child, depth + 1));
                        }
                    }
                }
            }

            unsafe { CFRelease(element as CFTypeRef) };

            if visited >= MAX_SCAN_NODES {
                break;
            }
        }

        if let Some((element, _, reason)) = best {
            return ScanOutcome::Found {
                element,
                visited,
                reason,
            };
        }

        ScanOutcome::NotFound {
            visited,
            candidates,
        }
    }

    fn collect_descendant_text(root: AXUIElementRef) -> Option<String> {
        const MAX_TEXT_SCAN_NODES: usize = 500;
        const MAX_TEXT_SCAN_DEPTH: usize = 10;

        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();
        let mut parts = Vec::new();
        let mut visited = 0usize;

        queue.push_back((retain_element(root), 0usize));

        while let Some((element, depth)) = queue.pop_front() {
            let hash = unsafe { CFHash(element as CFTypeRef) };
            if !seen.insert(hash) {
                unsafe { CFRelease(element as CFTypeRef) };
                continue;
            }

            visited += 1;

            if let Some(text) = readable_text_for_descendant(element) {
                push_unique_text(&mut parts, text);
            }

            if depth < MAX_TEXT_SCAN_DEPTH && visited < MAX_TEXT_SCAN_NODES {
                for attr in CHILD_RELATION_ATTRIBUTES {
                    if let Ok(children) = copy_attr_related_elements(element, attr) {
                        for child in children {
                            if queue.len() + visited >= MAX_TEXT_SCAN_NODES {
                                unsafe { CFRelease(child as CFTypeRef) };
                                continue;
                            }
                            queue.push_back((child, depth + 1));
                        }
                    }
                }
            }

            unsafe { CFRelease(element as CFTypeRef) };

            if visited >= MAX_TEXT_SCAN_NODES {
                break;
            }
        }

        let text = parts.join("\n");
        (!text.trim().is_empty()).then_some(text)
    }

    fn readable_text_for_descendant(element: AXUIElementRef) -> Option<String> {
        let role = copy_attr_string(element, "AXRole").ok().flatten();
        let subrole = copy_attr_string(element, "AXSubrole").ok().flatten();
        let role_is_text = is_text_role(role.as_deref(), subrole.as_deref());
        let role_name = role.as_deref().unwrap_or_default();
        let is_static_text = role_name == "AXStaticText";
        let selected_range_available = copy_attr_cf_range(element, "AXSelectedTextRange")
            .ok()
            .flatten()
            .is_some();
        let editable = copy_attr_bool(element, "AXIsEditable").ok().flatten() == Some(true);

        if role_is_text || selected_range_available || editable {
            if let Some(text) = readable_text_for_interactive_element(element) {
                return Some(text);
            }
        }

        if is_static_text {
            if let Some(text) =
                copy_first_non_empty_attr_string(element, &["AXValue", "AXTitle", "AXDescription"])
            {
                return Some(text);
            }
        }

        text_from_marker_apis(element)
            .map(|text| text.text)
            .filter(|text| !text.trim().is_empty())
    }

    fn readable_text_for_interactive_element(element: AXUIElementRef) -> Option<String> {
        copy_first_non_empty_attr_string(element, TEXT_VALUE_ATTRIBUTES)
            .or_else(|| text_from_marker_apis(element).map(|text| text.text))
    }

    fn terminal_context_text(
        element: AXUIElementRef,
        value: &str,
        visible_range: Option<AxRange>,
        app_name: Option<&str>,
        bundle_id: Option<&str>,
    ) -> TerminalContextText {
        if looks_like_iterm2_app(app_name, bundle_id) {
            match iterm2_current_session_contents() {
                Ok(text) if !text.trim().is_empty() => {
                    let (text, truncated) = terminal_tail_context(&text);
                    return TerminalContextText {
                        text,
                        truncated,
                        source: "iTerm2 AppleScript current session contents".to_string(),
                    };
                }
                Ok(_) => {}
                Err(error) => {
                    if let Some(context) = terminal_context_from_ax_visible_range(
                        element,
                        value,
                        visible_range,
                        Some(error),
                    ) {
                        return context;
                    }
                }
            }
        }

        if let Some(context) =
            terminal_context_from_ax_visible_range(element, value, visible_range, None)
        {
            return context;
        }

        let (text, truncated) = terminal_tail_context(value);
        TerminalContextText {
            text,
            truncated,
            source: "AXValue tail fallback".to_string(),
        }
    }

    fn terminal_context_from_ax_visible_range(
        element: AXUIElementRef,
        value: &str,
        visible_range: Option<AxRange>,
        fallback_reason: Option<String>,
    ) -> Option<TerminalContextText> {
        if let Some(range) = visible_range.filter(|range| range.length > 0) {
            if let Some(text) =
                string_for_range(element, range).filter(|text| !text.trim().is_empty())
            {
                let (text, truncated) = terminal_tail_context(&text);
                return Some(TerminalContextText {
                    text,
                    truncated,
                    source: terminal_source_with_fallback(
                        "AXVisibleCharacterRange/AXStringForRange",
                        fallback_reason.as_deref(),
                    ),
                });
            }

            if let Ok((_, text, _)) = split_by_utf16_range(value, range.location, range.length) {
                if !text.trim().is_empty() {
                    let (text, truncated) = terminal_tail_context(&text);
                    return Some(TerminalContextText {
                        text,
                        truncated,
                        source: terminal_source_with_fallback(
                            "AXVisibleCharacterRange",
                            fallback_reason.as_deref(),
                        ),
                    });
                }
            }
        }

        None
    }

    fn terminal_source_with_fallback(source: &str, fallback_reason: Option<&str>) -> String {
        match fallback_reason {
            Some(reason) => {
                format!("{source} after iTerm2 AppleScript failed: {reason}")
            }
            None => source.to_string(),
        }
    }

    fn iterm2_current_session_contents() -> Result<String, String> {
        let script = r#"tell application id "com.googlecode.iterm2"
if (count of windows) is 0 then return ""
tell current session of current window
return contents
end tell
end tell"#;

        run_osascript(script, Duration::from_millis(700))
    }

    fn run_osascript(script: &str, timeout: Duration) -> Result<String, String> {
        let mut child = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(script)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start osascript: {error}"))?;

        let started_at = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    let output = child
                        .wait_with_output()
                        .map_err(|error| format!("failed to read osascript output: {error}"))?;
                    if output.status.success() {
                        return Ok(String::from_utf8_lossy(&output.stdout)
                            .trim_end_matches('\n')
                            .to_string());
                    }

                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return Err(if stderr.is_empty() {
                        format!("osascript exited with {}", output.status)
                    } else {
                        stderr
                    });
                }
                Ok(None) if started_at.elapsed() >= timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("osascript timed out".to_string());
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => return Err(format!("failed to poll osascript: {error}")),
            }
        }
    }

    fn string_for_range(element: AXUIElementRef, range: AxRange) -> Option<String> {
        let cf_range = cf_range_for_ax_range(range)?;
        let range_value = unsafe {
            AXValueCreate(
                AX_VALUE_CF_RANGE_TYPE,
                &cf_range as *const CFRange as *const c_void,
            )
        };
        if range_value.is_null() {
            return None;
        }

        let text = copy_parameterized_string(element, "AXStringForRange", range_value as CFTypeRef)
            .ok()
            .flatten();
        unsafe { CFRelease(range_value as CFTypeRef) };
        text
    }

    fn cf_range_for_ax_range(range: AxRange) -> Option<CFRange> {
        if range.location < 0 || range.length < 0 {
            return None;
        }

        Some(CFRange {
            location: range.location.try_into().ok()?,
            length: range.length.try_into().ok()?,
        })
    }

    fn text_from_marker_apis(element: AXUIElementRef) -> Option<MarkerText> {
        let selected_range = copy_attr_text_marker_range(element, "AXSelectedTextMarkerRange")
            .ok()
            .flatten();
        let selected_indices = selected_range.and_then(|range| {
            let indices = text_marker_range_indices(element, range);
            unsafe { CFRelease(range as CFTypeRef) };
            indices
        });

        for range_source in [
            text_marker_range_for_ui_element(element),
            text_marker_range_from_start_end(element),
            text_marker_range_for_current_paragraph(element),
        ] {
            let Some(range) = range_source else {
                continue;
            };

            let text = string_for_text_marker_range(element, range);
            unsafe { CFRelease(range as CFTypeRef) };
            if let Some(text) = text.filter(|value| !value.trim().is_empty()) {
                return Some(MarkerText {
                    text,
                    range: selected_indices,
                });
            }
        }

        None
    }

    fn push_unique_text(parts: &mut Vec<String>, text: String) {
        let normalized = text.trim();
        if normalized.is_empty() {
            return;
        }

        if parts
            .last()
            .map(|last| last.trim() == normalized)
            .unwrap_or(false)
        {
            return;
        }

        parts.push(text);
    }

    fn text_marker_range_for_ui_element(element: AXUIElementRef) -> Option<AXTextMarkerRangeRef> {
        copy_parameterized_text_marker_range(
            element,
            "AXTextMarkerRangeForUIElement",
            element as CFTypeRef,
        )
        .ok()
        .flatten()
    }

    fn text_marker_range_from_start_end(element: AXUIElementRef) -> Option<AXTextMarkerRangeRef> {
        let start = copy_attr_text_marker(element, "AXStartTextMarker")
            .ok()
            .flatten()?;
        let end = copy_attr_text_marker(element, "AXEndTextMarker")
            .ok()
            .flatten()?;

        let range = unsafe { AXTextMarkerRangeCreate(kCFAllocatorDefault, start, end) };
        unsafe {
            CFRelease(start as CFTypeRef);
            CFRelease(end as CFTypeRef);
        }

        (!range.is_null()).then_some(range)
    }

    fn text_marker_range_for_current_paragraph(
        element: AXUIElementRef,
    ) -> Option<AXTextMarkerRangeRef> {
        let selected_range = copy_attr_text_marker_range(element, "AXSelectedTextMarkerRange")
            .ok()
            .flatten()?;
        let marker = unsafe { AXTextMarkerRangeCopyStartMarker(selected_range) };
        unsafe { CFRelease(selected_range as CFTypeRef) };

        if marker.is_null() {
            return None;
        }

        let range = copy_parameterized_text_marker_range(
            element,
            "AXParagraphTextMarkerRangeForTextMarker",
            marker as CFTypeRef,
        )
        .ok()
        .flatten();
        unsafe { CFRelease(marker as CFTypeRef) };
        range
    }

    fn string_for_text_marker_range(
        element: AXUIElementRef,
        range: AXTextMarkerRangeRef,
    ) -> Option<String> {
        copy_parameterized_string(element, "AXStringForTextMarkerRange", range as CFTypeRef)
            .ok()
            .flatten()
            .or_else(|| {
                copy_parameterized_string(
                    element,
                    "AXAttributedStringForTextMarkerRange",
                    range as CFTypeRef,
                )
                .ok()
                .flatten()
            })
    }

    fn text_marker_range_indices(
        element: AXUIElementRef,
        range: AXTextMarkerRangeRef,
    ) -> Option<AxRange> {
        let start = unsafe { AXTextMarkerRangeCopyStartMarker(range) };
        let end = unsafe { AXTextMarkerRangeCopyEndMarker(range) };
        if start.is_null() || end.is_null() {
            release_cf_if_not_null(start as CFTypeRef);
            release_cf_if_not_null(end as CFTypeRef);
            return None;
        }

        let start_index =
            copy_parameterized_number(element, "AXIndexForTextMarker", start as CFTypeRef)
                .ok()
                .flatten();
        let end_index =
            copy_parameterized_number(element, "AXIndexForTextMarker", end as CFTypeRef)
                .ok()
                .flatten();

        unsafe {
            CFRelease(start as CFTypeRef);
            CFRelease(end as CFTypeRef);
        }

        let (start_index, end_index) = (start_index?, end_index?);
        if end_index < start_index {
            return None;
        }

        Some(AxRange {
            location: start_index,
            length: end_index - start_index,
        })
    }

    fn find_focused_element(
        system: AXUIElementRef,
        focused_app: Option<AXUIElementRef>,
        frontmost_app: Option<AXUIElementRef>,
        focused_window: Option<AXUIElementRef>,
        frontmost_window: Option<AXUIElementRef>,
        mut failures: Vec<String>,
    ) -> Result<FocusedElement, Vec<String>> {
        for (label, element) in [
            ("system", Some(system)),
            ("focused app", focused_app),
            ("frontmost app", frontmost_app),
            ("focused window", focused_window),
            ("frontmost window", frontmost_window),
        ] {
            let Some(element) = element else {
                continue;
            };

            match copy_attr_element(element, "AXFocusedUIElement") {
                Ok(Some(focused_element)) => {
                    return Ok(FocusedElement {
                        element: focused_element,
                        source: label.to_string(),
                    });
                }
                Ok(None) => failures.push(format!("{label}: no value")),
                Err(error) => {
                    failures.push(format!("{label}: {}", ax_error_description(error)));
                }
            }
        }

        for (label, element) in [
            ("frontmost window tree", frontmost_window),
            ("focused window tree", focused_window),
            ("frontmost app tree", frontmost_app),
            ("focused app tree", focused_app),
        ] {
            let Some(element) = element else {
                continue;
            };

            match scan_for_context_element(element) {
                ScanOutcome::Found {
                    element,
                    visited,
                    reason,
                } => {
                    return Ok(FocusedElement {
                        element,
                        source: format!("{label} ({reason}, visited {visited} nodes)"),
                    });
                }
                ScanOutcome::NotFound {
                    visited,
                    candidates,
                } => {
                    failures.push(format!(
                        "{label}: no focused/editable descendant found after {visited} nodes ({candidates} candidates)"
                    ));
                }
            }
        }

        if failures.is_empty() {
            failures.push("no focused application or window available".to_string());
        }

        Err(failures)
    }

    fn ax_error_description(error: AXError) -> String {
        format!("{} ({error})", ax_error_name(error))
    }

    fn ax_error_name(error: AXError) -> &'static str {
        match error {
            AX_ERROR_SUCCESS => "kAXErrorSuccess",
            AX_ERROR_FAILURE => "kAXErrorFailure",
            AX_ERROR_ILLEGAL_ARGUMENT => "kAXErrorIllegalArgument",
            AX_ERROR_INVALID_UI_ELEMENT => "kAXErrorInvalidUIElement",
            AX_ERROR_INVALID_UI_ELEMENT_OBSERVER => "kAXErrorInvalidUIElementObserver",
            AX_ERROR_CANNOT_COMPLETE => "kAXErrorCannotComplete",
            AX_ERROR_ATTRIBUTE_UNSUPPORTED => "kAXErrorAttributeUnsupported",
            AX_ERROR_ACTION_UNSUPPORTED => "kAXErrorActionUnsupported",
            AX_ERROR_NOTIFICATION_UNSUPPORTED => "kAXErrorNotificationUnsupported",
            AX_ERROR_NOT_IMPLEMENTED => "kAXErrorNotImplemented",
            AX_ERROR_NOTIFICATION_ALREADY_REGISTERED => "kAXErrorNotificationAlreadyRegistered",
            AX_ERROR_NOTIFICATION_NOT_REGISTERED => "kAXErrorNotificationNotRegistered",
            AX_ERROR_API_DISABLED => "kAXErrorAPIDisabled",
            AX_ERROR_NO_VALUE => "kAXErrorNoValue",
            AX_ERROR_PARAMETERIZED_ATTRIBUTE_UNSUPPORTED => {
                "kAXErrorParameterizedAttributeUnsupported"
            }
            AX_ERROR_NOT_ENOUGH_PRECISION => "kAXErrorNotEnoughPrecision",
            _ => "unknown AXError",
        }
    }

    fn copy_attr_element(
        element: AXUIElementRef,
        attr: &str,
    ) -> Result<Option<AXUIElementRef>, AXError> {
        unsafe {
            let value = copy_attr_cf(element, attr)?;
            Ok(Some(value as AXUIElementRef))
        }
    }

    fn copy_attr_string(element: AXUIElementRef, attr: &str) -> Result<Option<String>, AXError> {
        unsafe {
            let value = copy_attr_cf(element, attr)?;
            if CFGetTypeID(value) != CFStringGetTypeID() {
                CFRelease(value);
                return Ok(None);
            }
            let text = cf_string_to_string(value as CFStringRef);
            CFRelease(value);
            Ok(text)
        }
    }

    fn copy_first_non_empty_attr_string(element: AXUIElementRef, attrs: &[&str]) -> Option<String> {
        attrs.iter().find_map(|attr| {
            copy_attr_string(element, attr)
                .ok()
                .flatten()
                .filter(|text| !text.trim().is_empty())
        })
    }

    fn copy_attr_bool(element: AXUIElementRef, attr: &str) -> Result<Option<bool>, AXError> {
        unsafe {
            let value = copy_attr_cf(element, attr)?;
            if CFGetTypeID(value) != CFBooleanGetTypeID() {
                CFRelease(value);
                return Ok(None);
            }
            let result = CFBooleanGetValue(value as CFBooleanRef);
            CFRelease(value);
            Ok(Some(result))
        }
    }

    fn copy_attr_number(element: AXUIElementRef, attr: &str) -> Result<Option<i64>, AXError> {
        unsafe {
            let value = copy_attr_cf(element, attr)?;
            if CFGetTypeID(value) != CFNumberGetTypeID() {
                CFRelease(value);
                return Ok(None);
            }
            let mut number = 0_i64;
            let ok = CFNumberGetValue(
                value as CFNumberRef,
                kCFNumberSInt64Type,
                &mut number as *mut i64 as *mut c_void,
            );
            CFRelease(value);
            Ok(ok.then_some(number))
        }
    }

    fn copy_attr_text_marker(
        element: AXUIElementRef,
        attr: &str,
    ) -> Result<Option<AXTextMarkerRef>, AXError> {
        unsafe {
            let value = copy_attr_cf(element, attr)?;
            if CFGetTypeID(value) != AXTextMarkerGetTypeID() {
                CFRelease(value);
                return Ok(None);
            }
            Ok(Some(value as AXTextMarkerRef))
        }
    }

    fn copy_attr_text_marker_range(
        element: AXUIElementRef,
        attr: &str,
    ) -> Result<Option<AXTextMarkerRangeRef>, AXError> {
        unsafe {
            let value = copy_attr_cf(element, attr)?;
            if CFGetTypeID(value) != AXTextMarkerRangeGetTypeID() {
                CFRelease(value);
                return Ok(None);
            }
            Ok(Some(value as AXTextMarkerRangeRef))
        }
    }

    fn copy_attr_related_elements(
        element: AXUIElementRef,
        attr: &str,
    ) -> Result<Vec<AXUIElementRef>, AXError> {
        unsafe {
            let value = copy_attr_cf(element, attr)?;
            let type_id = CFGetTypeID(value);
            if type_id == AXUIElementGetTypeID() {
                let related = CFRetain(value) as AXUIElementRef;
                CFRelease(value);
                return Ok(vec![related]);
            }

            if type_id != core_foundation_sys::array::CFArrayGetTypeID() {
                CFRelease(value);
                return Ok(Vec::new());
            }

            let array = value as CFArrayRef;
            let count = CFArrayGetCount(array);
            let mut elements = Vec::with_capacity(count.max(0) as usize);
            for index in 0..count {
                let child = CFArrayGetValueAtIndex(array, index) as CFTypeRef;
                if child.is_null() || CFGetTypeID(child) != AXUIElementGetTypeID() {
                    continue;
                }
                elements.push(CFRetain(child) as AXUIElementRef);
            }
            CFRelease(value);
            Ok(elements)
        }
    }

    fn copy_parameterized_number(
        element: AXUIElementRef,
        attr: &str,
        parameter: CFTypeRef,
    ) -> Result<Option<i64>, AXError> {
        unsafe {
            let value = copy_parameterized_cf(element, attr, parameter)?;
            if CFGetTypeID(value) != CFNumberGetTypeID() {
                CFRelease(value);
                return Ok(None);
            }
            let mut number = 0_i64;
            let ok = CFNumberGetValue(
                value as CFNumberRef,
                kCFNumberSInt64Type,
                &mut number as *mut i64 as *mut c_void,
            );
            CFRelease(value);
            Ok(ok.then_some(number))
        }
    }

    fn copy_parameterized_string(
        element: AXUIElementRef,
        attr: &str,
        parameter: CFTypeRef,
    ) -> Result<Option<String>, AXError> {
        unsafe {
            let value = copy_parameterized_cf(element, attr, parameter)?;
            let type_id = CFGetTypeID(value);
            if type_id == CFStringGetTypeID() {
                let text = cf_string_to_string(value as CFStringRef);
                CFRelease(value);
                return Ok(text);
            }

            if type_id == CFAttributedStringGetTypeID() {
                let text_ref = CFAttributedStringGetString(value as CFAttributedStringRef);
                let text = if text_ref.is_null() {
                    None
                } else {
                    cf_string_to_string(text_ref)
                };
                CFRelease(value);
                return Ok(text);
            }

            CFRelease(value);
            Ok(None)
        }
    }

    fn copy_parameterized_text_marker_range(
        element: AXUIElementRef,
        attr: &str,
        parameter: CFTypeRef,
    ) -> Result<Option<AXTextMarkerRangeRef>, AXError> {
        unsafe {
            let value = copy_parameterized_cf(element, attr, parameter)?;
            if CFGetTypeID(value) != AXTextMarkerRangeGetTypeID() {
                CFRelease(value);
                return Ok(None);
            }
            Ok(Some(value as AXTextMarkerRangeRef))
        }
    }

    fn copy_attr_cf_range(element: AXUIElementRef, attr: &str) -> Result<Option<AxRange>, AXError> {
        unsafe {
            let value = copy_attr_cf(element, attr)?;
            if CFGetTypeID(value) != AXValueGetTypeID() {
                CFRelease(value);
                return Ok(None);
            }
            let ax_value = value as AXValueRef;
            if AXValueGetType(ax_value) != AX_VALUE_CF_RANGE_TYPE {
                CFRelease(value);
                return Ok(None);
            }

            let mut range = CFRange {
                location: 0,
                length: 0,
            };
            let ok = AXValueGetValue(
                ax_value,
                AX_VALUE_CF_RANGE_TYPE,
                &mut range as *mut CFRange as *mut c_void,
            ) != 0;
            CFRelease(value);
            Ok(ok.then_some(AxRange {
                location: range.location as i64,
                length: range.length as i64,
            }))
        }
    }

    fn set_attr_bool(element: AXUIElementRef, attr: &str, value: bool) -> Result<(), AXError> {
        unsafe {
            let attr_ref = create_cf_string(attr);
            if attr_ref.is_null() {
                return Err(-1);
            }
            let bool_ref = if value {
                kCFBooleanTrue as CFTypeRef
            } else {
                core_foundation_sys::number::kCFBooleanFalse as CFTypeRef
            };
            let result = AXUIElementSetAttributeValue(element, attr_ref, bool_ref);
            CFRelease(attr_ref as CFTypeRef);
            if result == AX_ERROR_SUCCESS {
                Ok(())
            } else {
                Err(result)
            }
        }
    }

    fn copy_attribute_names(element: AXUIElementRef) -> Result<Vec<String>, AXError> {
        let mut names: CFArrayRef = ptr::null();
        let result = unsafe { AXUIElementCopyAttributeNames(element, &mut names) };
        if result != AX_ERROR_SUCCESS || names.is_null() {
            return Err(result);
        }

        let count = unsafe { CFArrayGetCount(names) };
        let mut result = Vec::with_capacity(count as usize);
        for index in 0..count {
            let value = unsafe { CFArrayGetValueAtIndex(names, index) as CFStringRef };
            if value.is_null() {
                continue;
            }
            if let Some(name) = unsafe { cf_string_to_string(value) } {
                result.push(name);
            }
        }
        unsafe { CFRelease(names as CFTypeRef) };
        Ok(result)
    }

    fn copy_parameterized_attribute_names(element: AXUIElementRef) -> Result<Vec<String>, AXError> {
        let mut names: CFArrayRef = ptr::null();
        let result = unsafe { AXUIElementCopyParameterizedAttributeNames(element, &mut names) };
        if result != AX_ERROR_SUCCESS || names.is_null() {
            return Err(result);
        }

        let count = unsafe { CFArrayGetCount(names) };
        let mut result = Vec::with_capacity(count as usize);
        for index in 0..count {
            let value = unsafe { CFArrayGetValueAtIndex(names, index) as CFStringRef };
            if value.is_null() {
                continue;
            }
            if let Some(name) = unsafe { cf_string_to_string(value) } {
                result.push(name);
            }
        }
        unsafe { CFRelease(names as CFTypeRef) };
        Ok(result)
    }

    unsafe fn copy_attr_cf(element: AXUIElementRef, attr: &str) -> Result<CFTypeRef, AXError> {
        let attr_ref = create_cf_string(attr);
        if attr_ref.is_null() {
            return Err(-1);
        }

        let mut value: CFTypeRef = ptr::null();
        let result = AXUIElementCopyAttributeValue(element, attr_ref, &mut value);
        CFRelease(attr_ref as CFTypeRef);

        if result != AX_ERROR_SUCCESS || value.is_null() {
            return Err(result);
        }

        Ok(value)
    }

    unsafe fn copy_parameterized_cf(
        element: AXUIElementRef,
        attr: &str,
        parameter: CFTypeRef,
    ) -> Result<CFTypeRef, AXError> {
        let attr_ref = create_cf_string(attr);
        if attr_ref.is_null() {
            return Err(-1);
        }

        let mut value: CFTypeRef = ptr::null();
        let result =
            AXUIElementCopyParameterizedAttributeValue(element, attr_ref, parameter, &mut value);
        CFRelease(attr_ref as CFTypeRef);

        if result != AX_ERROR_SUCCESS || value.is_null() {
            return Err(result);
        }

        Ok(value)
    }

    unsafe fn create_cf_string(value: &str) -> CFStringRef {
        let Ok(c_string) = CString::new(value) else {
            return ptr::null();
        };
        CFStringCreateWithCString(
            kCFAllocatorDefault,
            c_string.as_ptr(),
            kCFStringEncodingUTF8,
        )
    }

    unsafe fn cf_string_to_string(value: CFStringRef) -> Option<String> {
        let length = core_foundation_sys::string::CFStringGetLength(value);
        let max_size = CFStringGetMaximumSizeForEncoding(length, kCFStringEncodingUTF8) + 1;
        if max_size <= 0 {
            return Some(String::new());
        }

        let mut buffer = vec![0 as c_char; max_size as usize];
        let ok = CFStringGetCString(value, buffer.as_mut_ptr(), max_size, kCFStringEncodingUTF8);
        if ok == 0 {
            return None;
        }

        Some(
            CStr::from_ptr(buffer.as_ptr())
                .to_string_lossy()
                .into_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_split_handles_ascii() {
        let (before, selected, after) =
            split_by_utf16_range("hello world", 6, 5).expect("split ascii");

        assert_eq!(before, "hello ");
        assert_eq!(selected, "world");
        assert_eq!(after, "");
    }

    #[test]
    fn utf16_split_handles_cjk_and_emoji() {
        let text = "你好🙂world";
        let emoji_offset = "你好".encode_utf16().count() as i64;
        let (before, selected, after) =
            split_by_utf16_range(text, emoji_offset, 2).expect("split emoji");

        assert_eq!(before, "你好");
        assert_eq!(selected, "🙂");
        assert_eq!(after, "world");
    }

    #[test]
    fn utf16_split_rejects_inside_surrogate_pair() {
        let text = "a🙂b";
        let err = split_by_utf16_range(text, 2, 0).expect_err("inside surrogate pair");

        assert!(err.contains("inside a character"));
    }

    #[test]
    fn utf16_split_handles_combining_mark_boundaries() {
        let text = "e\u{301}clair";
        let (before, selected, after) =
            split_by_utf16_range(text, 1, 1).expect("split combining mark");

        assert_eq!(before, "e");
        assert_eq!(selected, "\u{301}");
        assert_eq!(after, "clair");
    }

    #[test]
    fn secure_field_detection_matches_role_and_label() {
        assert!(looks_like_secure_field(&[
            Some("AXSecureTextField".to_string()),
            None,
            None,
        ]));
        assert!(looks_like_secure_field(&[
            Some("AXTextField".to_string()),
            Some("Password".to_string()),
        ]));
        assert!(!looks_like_secure_field(&[
            Some("AXTextArea".to_string()),
            Some("Message".to_string()),
        ]));
    }

    #[test]
    fn terminal_app_detection_matches_common_terminals() {
        assert!(looks_like_terminal_app(
            Some("iTerm2"),
            Some("com.googlecode.iterm2")
        ));
        assert!(looks_like_iterm2_app(
            Some("iTerm2"),
            Some("com.googlecode.iterm2")
        ));
        assert!(looks_like_terminal_app(
            Some("Terminal"),
            Some("com.apple.Terminal")
        ));
        assert!(!looks_like_iterm2_app(
            Some("Terminal"),
            Some("com.apple.Terminal")
        ));
        assert!(looks_like_terminal_app(
            Some("Ghostty"),
            Some("com.mitchellh.ghostty")
        ));
        assert!(!looks_like_terminal_app(
            Some("Safari"),
            Some("com.apple.Safari")
        ));
    }

    #[test]
    fn terminal_tail_context_keeps_recent_lines() {
        let text = (0..140)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");

        let (tail, truncated) = terminal_tail_context(&text);

        assert!(truncated);
        assert!(!tail.contains("line 0\n"));
        assert!(tail.starts_with("line 110"));
        assert!(tail.ends_with("line 139"));
    }

    #[test]
    fn terminal_tail_context_truncates_long_single_line_from_end() {
        let text = format!("{}tail", "x".repeat(TERMINAL_CONTEXT_MAX_CHARS + 20));

        let (tail, truncated) = terminal_tail_context(&text);

        assert!(truncated);
        assert_eq!(tail.chars().count(), TERMINAL_CONTEXT_MAX_CHARS);
        assert!(tail.ends_with("tail"));
    }

    #[test]
    fn status_and_confidence_round_trip_from_db_strings() {
        assert_eq!(
            ContextProbeStatus::from_db(ContextProbeStatus::Success.as_str()),
            ContextProbeStatus::Success
        );
        assert_eq!(
            ContextProbeConfidence::from_db(ContextProbeConfidence::High.as_str()),
            ContextProbeConfidence::High
        );
        assert_eq!(
            ContextProbeStatus::from_db("unknown"),
            ContextProbeStatus::Error
        );
        assert_eq!(
            ContextProbeConfidence::from_db("unknown"),
            ContextProbeConfidence::None
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_capture_returns_unsupported_platform() {
        let run = capture_focused_context("test".to_string());

        assert_eq!(run.status, ContextProbeStatus::UnsupportedPlatform);
        assert_eq!(run.confidence, ContextProbeConfidence::None);
    }
}
