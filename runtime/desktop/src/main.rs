#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod assets;

use std::error::Error;
use std::io;
use std::rc::Rc;
use std::time::{Duration, Instant};

use assets::{AppAssets, START_URL};
use embedder_traits::EventLoopWaker;
use euclid::Scale;
use keyboard_types::{Code, Key, KeyState, Location, Modifiers, NamedKey};
use servo::{
    Cursor, InputEvent, KeyboardEvent, MouseButton, MouseButtonAction, MouseButtonEvent,
    MouseMoveEvent, Preferences, RenderingContext, Servo, ServoBuilder, WebResourceLoad, WebView,
    WebViewBuilder, WebViewDelegate, WheelDelta, WheelEvent, WheelMode, WindowRenderingContext,
};
use url::Url;
use webrender_api::units::DevicePoint;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, KeyLocation, ModifiersState, PhysicalKey};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{CursorIcon, Window, WindowId};

const FRAME_INTERVAL: Duration = Duration::from_millis(16);
const WHEEL_PIXELS_PER_LINE: f64 = 38.0;

fn main() {
    if let Err(error) = run() {
        report_error(&error.to_string());
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| io::Error::other("Could not initialize the TLS provider"))?;
    let event_loop = EventLoop::with_user_event().build()?;
    let mut app = App {
        assets: Rc::new(AppAssets::beside_executable()?),
        waker: Waker(event_loop.create_proxy()),
        running: None,
        error: None,
        pointer: DevicePoint::default(),
        modifiers: ModifiersState::empty(),
        next_frame: Instant::now(),
    };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(windows)]
fn report_error(message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MessageBoxW};
    let message: Vec<_> = message.encode_utf16().chain(Some(0)).collect();
    let title: Vec<_> = "Proton.js".encode_utf16().chain(Some(0)).collect();
    // Both strings are NUL-terminated and live until MessageBoxW returns.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_ICONERROR,
        )
    };
}

#[cfg(not(windows))]
fn report_error(message: &str) {
    eprintln!("Proton.js: {message}");
}

#[derive(Clone)]
struct Waker(EventLoopProxy<()>);

impl EventLoopWaker for Waker {
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(self.clone())
    }

    fn wake(&self) {
        let _ = self.0.send_event(());
    }
}

struct WindowDelegate {
    window: Rc<Window>,
    assets: Rc<AppAssets>,
}

impl WebViewDelegate for WindowDelegate {
    fn notify_new_frame_ready(&self, _: WebView) {
        self.window.request_redraw();
    }

    fn notify_page_title_changed(&self, _: WebView, title: Option<String>) {
        self.window
            .set_title(title.as_deref().unwrap_or("Proton.js"));
    }

    fn notify_cursor_changed(&self, _: WebView, cursor: Cursor) {
        self.window.set_cursor_visible(cursor != Cursor::None);
        self.window.set_cursor(match cursor {
            Cursor::Pointer => CursorIcon::Pointer,
            Cursor::Text => CursorIcon::Text,
            Cursor::Crosshair => CursorIcon::Crosshair,
            Cursor::Wait => CursorIcon::Wait,
            _ => CursorIcon::Default,
        });
    }

    fn load_web_resource(&self, _: WebView, load: WebResourceLoad) {
        self.assets.respond(load);
    }
}

struct Runtime {
    webview: WebView,
    servo: Servo,
    context: Rc<WindowRenderingContext>,
    window: Rc<Window>,
}

impl Runtime {
    fn open(
        event_loop: &ActiveEventLoop,
        waker: Waker,
        assets: Rc<AppAssets>,
    ) -> Result<Self, Box<dyn Error>> {
        let window = Rc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Proton.js")
                    .with_inner_size(LogicalSize::new(1080, 760))
                    .with_min_inner_size(LogicalSize::new(360, 340)),
            )?,
        );
        let context = Rc::new(
            WindowRenderingContext::new(
                event_loop.display_handle()?,
                window.window_handle()?,
                window.inner_size(),
            )
            .map_err(|error| {
                io::Error::other(format!("Servo graphics context error ({error:?})"))
            })?,
        );
        context.make_current().map_err(|error| {
            io::Error::other(format!("Servo graphics context error ({error:?})"))
        })?;
        let servo = ServoBuilder::default()
            .preferences(Preferences {
                dom_webgl2_enabled: true,
                dom_webgpu_enabled: true,
                ..Preferences::default()
            })
            .event_loop_waker(Box::new(waker))
            .build();
        servo.setup_logging();
        servo.set_delegate(assets.clone());
        let webview = WebViewBuilder::new(&servo, context.clone())
            .url(Url::parse(START_URL)?)
            .hidpi_scale_factor(Scale::new(window.scale_factor() as f32))
            .delegate(Rc::new(WindowDelegate {
                window: window.clone(),
                assets,
            }))
            .build();
        servo.spin_event_loop();
        window.request_redraw();
        Ok(Self {
            webview,
            servo,
            context,
            window,
        })
    }
}

struct App {
    assets: Rc<AppAssets>,
    waker: Waker,
    running: Option<Runtime>,
    error: Option<Box<dyn Error>>,
    pointer: DevicePoint,
    modifiers: ModifiersState,
    next_frame: Instant,
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.running.is_some() {
            return;
        }
        match Runtime::open(event_loop, self.waker.clone(), self.assets.clone()) {
            Ok(runtime) => self.running = Some(runtime),
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, _: &ActiveEventLoop, _: ()) {
        if let Some(runtime) = &self.running {
            runtime.servo.spin_event_loop();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(runtime) = &self.running
            && runtime.webview.animating()
            && !runtime.window.is_minimized().unwrap_or(false)
        {
            let now = Instant::now();
            if now >= self.next_frame {
                runtime.servo.spin_event_loop();
                self.next_frame = now + FRAME_INTERVAL;
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(runtime) = &self.running else { return };
        if runtime.window.id() != id {
            return;
        }
        let webview = &runtime.webview;
        let input = match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::RedrawRequested => {
                webview.paint();
                runtime.context.present();
                None
            }
            WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                webview.resize(size);
                runtime.window.request_redraw();
                None
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                webview.set_hidpi_scale_factor(Scale::new(scale_factor as f32));
                None
            }
            WindowEvent::Focused(focused) => {
                webview.set_focused(focused);
                None
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = DevicePoint::new(position.x as f32, position.y as f32);
                Some(InputEvent::MouseMove(MouseMoveEvent::new(
                    self.pointer.into(),
                )))
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Primary,
                    winit::event::MouseButton::Middle => MouseButton::Auxiliary,
                    winit::event::MouseButton::Right => MouseButton::Secondary,
                    winit::event::MouseButton::Back => MouseButton::Back,
                    winit::event::MouseButton::Forward => MouseButton::Forward,
                    winit::event::MouseButton::Other(value) => MouseButton::Other(value),
                };
                let action = if state.is_pressed() {
                    MouseButtonAction::Down
                } else {
                    MouseButtonAction::Up
                };
                Some(InputEvent::MouseButton(MouseButtonEvent::new(
                    action,
                    button,
                    self.pointer.into(),
                )))
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        let distance = WHEEL_PIXELS_PER_LINE * runtime.window.scale_factor();
                        (f64::from(x) * distance, f64::from(y) * distance)
                    }
                    MouseScrollDelta::PixelDelta(position) => (position.x, position.y),
                };
                Some(InputEvent::Wheel(WheelEvent::new(
                    WheelDelta {
                        x,
                        y,
                        z: 0.0,
                        mode: WheelMode::DeltaPixel,
                    },
                    self.pointer.into(),
                )))
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
                None
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => Some(InputEvent::Keyboard(keyboard_event(event, self.modifiers))),
            _ => None,
        };
        if let Some(input) = input {
            webview.notify_input_event(input);
        }
        runtime.servo.spin_event_loop();
    }
}

fn keyboard_event(event: KeyEvent, state: ModifiersState) -> KeyboardEvent {
    let key = match event.logical_key {
        WinitKey::Character(text) => Key::Character(text.to_string()),
        WinitKey::Named(winit::keyboard::NamedKey::Space) => Key::Character(" ".to_string()),
        WinitKey::Named(winit::keyboard::NamedKey::Super) => Key::Named(NamedKey::Meta),
        // Winit and keyboard-types use the same UI Events names for standard keys.
        WinitKey::Named(key) => Key::Named(format!("{key:?}").parse().unwrap_or_default()),
        _ => Key::Named(NamedKey::Unidentified),
    };
    let code = match event.physical_key {
        PhysicalKey::Code(code) => format!("{code:?}").parse().unwrap_or_default(),
        PhysicalKey::Unidentified(_) => Code::Unidentified,
    };
    let location = match event.location {
        KeyLocation::Standard => Location::Standard,
        KeyLocation::Left => Location::Left,
        KeyLocation::Right => Location::Right,
        KeyLocation::Numpad => Location::Numpad,
    };
    let mut modifiers = Modifiers::empty();
    modifiers.set(Modifiers::SHIFT, state.shift_key());
    modifiers.set(Modifiers::CONTROL, state.control_key());
    modifiers.set(Modifiers::ALT, state.alt_key());
    modifiers.set(Modifiers::META, state.super_key());
    KeyboardEvent::new(keyboard_types::KeyboardEvent {
        state: match event.state {
            ElementState::Pressed => KeyState::Down,
            ElementState::Released => KeyState::Up,
        },
        key,
        code,
        location,
        modifiers,
        repeat: event.repeat,
        is_composing: false,
    })
}
