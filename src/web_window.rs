//! Browser input translated to the small window API used by the game loop.
#![allow(dead_code, non_upper_case_globals)]
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/web/runtime.js")]
extern "C" {
    fn setup();
    #[wasm_bindgen(js_name = gameCanvas)]
    fn game_canvas() -> web_sys::HtmlCanvasElement;
    #[wasm_bindgen(js_name = drainEvents)]
    fn drain_events() -> js_sys::Array;
    #[wasm_bindgen(js_name = keyDown)]
    fn key_down(key: i32) -> bool;
    #[wasm_bindgen(js_name = cursorPosition)]
    fn cursor_position() -> js_sys::Array;
    #[wasm_bindgen(js_name = cursorMode)]
    fn cursor_mode(locked: bool);
    #[wasm_bindgen(js_name = gamepadState)]
    fn gamepad_state(index: u32) -> JsValue;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Release,
    Press,
    Repeat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Key {
    Unknown = -1,
    Space = 32,
    Num1 = 49,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    A = 65,
    D = 68,
    E = 69,
    Q = 81,
    S = 83,
    W = 87,
    Escape = 256,
    F3 = 292,
    F6 = 295,
    F11 = 300,
    LeftShift = 340,
    LeftControl = 341,
}

impl Key {
    fn from_code(code: i32) -> Self {
        match code {
            32 => Self::Space,
            49 => Self::Num1,
            50 => Self::Num2,
            51 => Self::Num3,
            52 => Self::Num4,
            53 => Self::Num5,
            54 => Self::Num6,
            55 => Self::Num7,
            56 => Self::Num8,
            57 => Self::Num9,
            65 => Self::A,
            68 => Self::D,
            69 => Self::E,
            81 => Self::Q,
            83 => Self::S,
            87 => Self::W,
            256 => Self::Escape,
            292 => Self::F3,
            295 => Self::F6,
            300 => Self::F11,
            340 => Self::LeftShift,
            341 => Self::LeftControl,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Modifiers(u8);
impl Modifiers {
    pub const Shift: Self = Self(1);
    pub const Control: Self = Self(2);
    pub fn empty() -> Self {
        Self(0)
    }
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

#[derive(Clone, Copy, Debug)]
pub enum WindowEvent {
    Key(Key, i32, Action, Modifiers),
    CursorPos(f64, f64),
    CursorDelta(f64, f64),
    MouseButton(i32, Action, Modifiers),
    Scroll(f64, f64),
    Size(i32, i32),
    Focus(bool),
}
pub const MouseButtonLeft: i32 = 0;
pub const MouseButtonRight: i32 = 1;
pub enum CursorMode {
    Normal,
    Disabled,
}
pub struct Events;
pub struct Glfw;
pub struct Window {
    canvas: web_sys::HtmlCanvasElement,
    closing: bool,
}

pub fn create() -> (Glfw, Window, Events) {
    setup();
    (
        Glfw,
        Window {
            canvas: game_canvas(),
            closing: false,
        },
        Events,
    )
}

impl Window {
    pub fn canvas(&self) -> web_sys::HtmlCanvasElement {
        self.canvas.clone()
    }
    pub fn get_framebuffer_size(&self) -> (i32, i32) {
        (self.canvas.width() as i32, self.canvas.height() as i32)
    }
    pub fn get_size(&self) -> (i32, i32) {
        self.get_framebuffer_size()
    }
    pub fn get_cursor_pos(&self) -> (f64, f64) {
        let pos = cursor_position();
        (
            pos.get(0).as_f64().unwrap_or(0.0),
            pos.get(1).as_f64().unwrap_or(0.0),
        )
    }
    pub fn get_key(&self, key: Key) -> Action {
        if key_down(key as i32) {
            Action::Press
        } else {
            Action::Release
        }
    }
    pub fn set_cursor_mode(&mut self, mode: CursorMode) {
        cursor_mode(matches!(mode, CursorMode::Disabled));
    }
    pub fn should_close(&self) -> bool {
        self.closing
    }
    pub fn set_should_close(&mut self, close: bool) {
        self.closing = close;
    }
    pub fn set_title(&mut self, title: &str) {
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            document.set_title(title);
        }
    }
}

impl Glfw {
    pub fn get_time(&self) -> f64 {
        web_sys::window().unwrap().performance().unwrap().now() / 1000.0
    }
    pub fn poll_events(&mut self) {}
    pub fn get_joystick(&self, id: JoystickId) -> Joystick {
        Joystick(gamepad_state(id as u32))
    }
}

pub fn flush_messages(_: &Events) -> std::vec::IntoIter<(f64, WindowEvent)> {
    drain_events()
        .iter()
        .filter_map(|event| {
            let row = js_sys::Array::from(&event);
            let n = |i| row.get(i).as_f64().unwrap_or(0.0);
            let action = if n(2) == 1.0 {
                Action::Press
            } else if n(2) == 2.0 {
                Action::Repeat
            } else {
                Action::Release
            };
            let mods = Modifiers(n(3) as u8);
            let event = match n(0) as i32 {
                0 => WindowEvent::Key(Key::from_code(n(1) as i32), 0, action, mods),
                1 => WindowEvent::CursorPos(n(1), n(2)),
                2 => WindowEvent::MouseButton(n(1) as i32, action, mods),
                3 => WindowEvent::Scroll(n(1), n(2)),
                4 => WindowEvent::Focus(false),
                5 => WindowEvent::CursorDelta(n(1), n(2)),
                _ => return None,
            };
            Some((0.0, event))
        })
        .collect::<Vec<_>>()
        .into_iter()
}

#[derive(Clone, Copy)]
pub enum JoystickId {
    Joystick1,
    Joystick2,
    Joystick3,
    Joystick4,
    Joystick5,
    Joystick6,
    Joystick7,
    Joystick8,
    Joystick9,
    Joystick10,
    Joystick11,
    Joystick12,
    Joystick13,
    Joystick14,
    Joystick15,
    Joystick16,
}
pub enum GamepadAxis {
    AxisLeftX,
    AxisLeftY,
    AxisRightX,
    AxisRightY,
    AxisLeftTrigger,
    AxisRightTrigger,
}
pub enum GamepadButton {
    ButtonA = 0,
    ButtonB = 1,
    ButtonLeftBumper = 4,
    ButtonRightBumper = 5,
    ButtonLeftThumb = 10,
    ButtonDpadLeft = 14,
    ButtonDpadRight = 15,
}
pub struct Joystick(JsValue);
pub struct GamepadState(js_sys::Array);
impl Joystick {
    pub fn is_present(&self) -> bool {
        !self.0.is_null() && !self.0.is_undefined()
    }
    pub fn get_gamepad_state(&self) -> Option<GamepadState> {
        self.is_present()
            .then(|| GamepadState(js_sys::Array::from(&self.0)))
    }
}
impl GamepadState {
    pub fn get_axis(&self, axis: GamepadAxis) -> f32 {
        self.0.get(axis as u32).as_f64().unwrap_or(0.0) as f32
    }
    pub fn get_button_state(&self, button: GamepadButton) -> Action {
        if self.0.get(6 + button as u32).as_bool().unwrap_or(false) {
            Action::Press
        } else {
            Action::Release
        }
    }
}
