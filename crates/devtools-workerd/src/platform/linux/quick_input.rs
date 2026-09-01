//! Linux 原生快速输入窗口：GTK Entry 实现，不创建 WebView。

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use devtools_core::{LanguageMode, Settings};
use gtk::prelude::*;
use gtk::{gdk, pango};
use tao::event_loop::EventLoopProxy;

use super::resolve_language;
use crate::UserEvent;

/// Linux 原生快速输入窗口使用 GTK，不创建 WebView。
pub struct QuickInputWindow {
    window: gtk::Window,
    entry: gtk::Entry,
    history: Rc<RefCell<Vec<String>>>,
    history_cursor: Rc<Cell<usize>>,
}

impl QuickInputWindow {
    pub fn new(
        proxy: EventLoopProxy<UserEvent>,
        initial_history: Vec<String>,
    ) -> Result<Self, String> {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("DevTools Quick Input");
        window.set_decorated(false);
        window.set_keep_above(true);
        window.set_skip_pager_hint(true);
        window.set_skip_taskbar_hint(true);
        window.set_type_hint(gdk::WindowTypeHint::Utility);
        window.set_default_size(560, 56);
        window.set_resizable(true);
        window.set_position(gtk::WindowPosition::Mouse);
        // X11 下窗口管理器仍可关闭 utility 窗口（Alt+F4）；拦截删除事件改为隐藏，
        // 避免控件销毁后再次唤出时对已销毁 widget 调用 show/present。
        window.connect_delete_event(|window, _| {
            window.hide();
            gtk::glib::Propagation::Stop
        });

        let entry = gtk::Entry::new();
        // GtkEntry 默认会根据当前文本自动计算自然宽度。固定其尺寸请求后再用 hexpand
        // 填满窗口，可避免输入法预编辑或长文本变化触发顶层窗口重复布局。
        entry.set_width_chars(1);
        entry.set_max_width_chars(1);
        entry.set_hexpand(true);
        entry.set_vexpand(true);
        entry.set_margin_top(8);
        entry.set_margin_bottom(8);
        entry.set_margin_start(12);
        entry.set_margin_end(12);
        configure_preedit_style(&entry);
        window.add(&entry);

        let history = Rc::new(RefCell::new(initial_history));
        let history_cursor = Rc::new(Cell::new(history.borrow().len()));

        let activate_window = window.clone();
        let activate_history = Rc::clone(&history);
        let activate_cursor = Rc::clone(&history_cursor);
        let activate_proxy = proxy;
        entry.connect_activate(move |entry| {
            let text = entry.text().to_string();
            activate_window.hide();
            entry.set_text("");
            if text.is_empty() {
                return;
            }
            activate_history.borrow_mut().push(text.clone());
            activate_cursor.set(activate_history.borrow().len());
            let _ = activate_proxy.send_event(UserEvent::QuickInputSubmitted { text });
        });

        let key_window = window.clone();
        let key_history = Rc::clone(&history);
        let key_cursor = Rc::clone(&history_cursor);
        entry.connect_key_press_event(move |entry, event| {
            let key = event.keyval();
            if key == gdk::keys::constants::Escape {
                key_window.hide();
                return gtk::glib::Propagation::Stop;
            }
            let values = key_history.borrow();
            if key == gdk::keys::constants::Up && !values.is_empty() {
                let next = key_cursor.get().saturating_sub(1);
                key_cursor.set(next);
                entry.set_text(&values[next]);
                entry.set_position(-1);
                return gtk::glib::Propagation::Stop;
            }
            if key == gdk::keys::constants::Down && !values.is_empty() {
                let next = (key_cursor.get() + 1).min(values.len());
                key_cursor.set(next);
                entry.set_text(values.get(next).map_or("", String::as_str));
                entry.set_position(-1);
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });

        Ok(Self {
            window,
            entry,
            history,
            history_cursor,
        })
    }

    pub fn show(&self, settings: &Settings) {
        if self.window.is_visible() {
            self.window.present();
            self.entry.grab_focus();
            return;
        }
        let (width, height, position) = bounded_geometry(settings);
        self.window.set_default_size(width, height);
        self.window.set_size_request(width, height);
        self.window.resize(width, height);
        if let Some((x, y)) = position {
            self.window.move_(x, y);
        } else {
            self.window.set_position(gtk::WindowPosition::Mouse);
        }
        self.entry
            .set_placeholder_text(Some(placeholder(settings.language)));
        self.entry.set_text("");
        self.history_cursor.set(self.history.borrow().len());
        self.window.show_all();
        self.window.present();
        self.entry.grab_focus();
    }
}

const PREEDIT_BACKGROUND_ALPHA: u16 = 0x3800;

/// 输入法预编辑只使用背景色标记，字体、字号、升降和下划线均继承输入框本身。
fn configure_preedit_style(entry: &gtk::Entry) {
    entry.connect_local("preedit-changed", true, |values| {
        if let (Ok(entry), Ok(preedit)) = (values[0].get::<gtk::Entry>(), values[1].get::<String>())
        {
            apply_preedit_style(&entry, &preedit);
        }
        None
    });
}

fn apply_preedit_style(entry: &gtk::Entry, preedit: &str) {
    if preedit.is_empty() {
        return;
    }

    let text = entry.text();
    let cursor = entry.position().max(0) as usize;
    let text_index = byte_index_at_char(&text, cursor);
    let start = entry.text_index_to_layout_index(text_index as i32).max(0) as u32;
    let end = start.saturating_add(preedit.len() as u32);
    let Some(layout) = entry.layout() else {
        return;
    };
    let attributes = layout
        .attributes()
        .and_then(|attributes| attributes.copy())
        .unwrap_or_default();

    // GtkEntry 会把输入法提供的属性拼入预编辑区间；移除该区间内的全部 IME
    // 属性，防止下划线、字体或 rise 等属性改变布局高度。跨越整个文本的主题属性
    // 不在此移除，因此普通文本继续完整跟随系统主题。
    let _ = attributes.filter(|attribute| {
        preedit_contains_attribute(start, end, attribute.start_index(), attribute.end_index())
    });

    let color = preedit_background(entry);
    let mut background = pango::AttrColor::new_background(
        pango_color_channel(color.red()),
        pango_color_channel(color.green()),
        pango_color_channel(color.blue()),
    );
    background.set_start_index(start);
    background.set_end_index(end);
    attributes.change(background);

    let mut alpha = pango::AttrInt::new_background_alpha(PREEDIT_BACKGROUND_ALPHA);
    alpha.set_start_index(start);
    alpha.set_end_index(end);
    attributes.change(alpha);

    layout.set_attributes(Some(&attributes));
    entry.queue_draw();
}

fn byte_index_at_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(index, _)| index)
}

fn preedit_contains_attribute(start: u32, end: u32, attr_start: u32, attr_end: u32) -> bool {
    attr_start >= start && attr_end <= end
}

fn preedit_background(entry: &gtk::Entry) -> gdk::RGBA {
    let context = entry.style_context();
    context
        .lookup_color("theme_selected_bg_color")
        .or_else(|| context.lookup_color("accent_bg_color"))
        .unwrap_or_else(|| gdk::RGBA::new(0.20, 0.48, 0.82, 1.0))
}

fn pango_color_channel(value: f64) -> u16 {
    (value.clamp(0.0, 1.0) * f64::from(u16::MAX)).round() as u16
}

fn bounded_geometry(settings: &Settings) -> (i32, i32, Option<(i32, i32)>) {
    let requested_width = settings.quick_input_width.clamp(240, 1600) as i32;
    let requested_height = settings.quick_input_height.clamp(40, 240) as i32;
    let Some(display) = gdk::Display::default() else {
        return (requested_width, requested_height, None);
    };
    let Some(pointer) = display.default_seat().and_then(|seat| seat.pointer()) else {
        return (requested_width, requested_height, None);
    };
    let (_, pointer_x, pointer_y) = pointer.position();
    let Some(monitor) = display.monitor_at_point(pointer_x, pointer_y) else {
        return (requested_width, requested_height, None);
    };
    let workarea = monitor.workarea();
    let width = requested_width.min((workarea.width() - 24).max(240));
    let height = requested_height.min((workarea.height() - 24).max(40));
    if is_wayland() {
        return (width, height, None);
    }
    let x = pointer_x
        .min(workarea.x() + workarea.width() - width - 12)
        .max(workarea.x() + 12);
    let y = (pointer_y + 16)
        .min(workarea.y() + workarea.height() - height - 12)
        .max(workarea.y() + 12);
    (width, height, Some((x, y)))
}

fn is_wayland() -> bool {
    env_var_present("WAYLAND_DISPLAY")
        && std::env::var("GDK_BACKEND").map_or(true, |value| value != "x11")
}

fn env_var_present(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn placeholder(language: LanguageMode) -> &'static str {
    match resolve_language(language) {
        LanguageMode::SimplifiedChinese => "输入内容，Enter 复制并关闭，↑↓ 历史",
        LanguageMode::TraditionalChinese => "輸入內容，Enter 複製並關閉，↑↓ 歷史",
        LanguageMode::System | LanguageMode::English => {
            "Type text, Enter to copy and close, ↑↓ for history"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_clipboard_only_quick_input_action() {
        assert!(placeholder(LanguageMode::SimplifiedChinese).contains("复制并关闭"));
        assert!(placeholder(LanguageMode::TraditionalChinese).contains("複製並關閉"));
        assert!(placeholder(LanguageMode::English).contains("copy and close"));
    }

    #[test]
    fn converts_character_cursor_to_utf8_byte_index() {
        assert_eq!(byte_index_at_char("a中文", 0), 0);
        assert_eq!(byte_index_at_char("a中文", 1), 1);
        assert_eq!(byte_index_at_char("a中文", 2), 4);
        assert_eq!(byte_index_at_char("a中文", 3), 7);
        assert_eq!(byte_index_at_char("a中文", 99), 7);
    }

    #[test]
    fn removes_only_attributes_owned_by_preedit_range() {
        assert!(preedit_contains_attribute(3, 8, 3, 8));
        assert!(preedit_contains_attribute(3, 8, 4, 7));
        assert!(!preedit_contains_attribute(3, 8, 0, u32::MAX));
        assert!(!preedit_contains_attribute(3, 8, 2, 5));
        assert!(!preedit_contains_attribute(3, 8, 7, 9));
    }

    #[test]
    fn converts_theme_colors_to_pango_channels() {
        assert_eq!(pango_color_channel(-1.0), 0);
        assert_eq!(pango_color_channel(0.5), 32_768);
        assert_eq!(pango_color_channel(2.0), u16::MAX);
    }
}
