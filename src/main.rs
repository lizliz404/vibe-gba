use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use minifb::{Key, Scale, Window, WindowOptions};
use vibe_gba::cartridge::load_rom_file;
use vibe_gba::gba::{Button, Gba};
use vibe_gba::{SCREEN_HEIGHT, SCREEN_WIDTH};

struct Args {
    rom: PathBuf,
    save: Option<PathBuf>,
    screenshot: Option<PathBuf>,
    save_state: Option<PathBuf>,
    load_state: Option<PathBuf>,
    frames: Option<u64>,
    turbo: bool,
    trace: bool,
    dump_state: bool,
    stop_pc: Option<u32>,
    stop_invalid: bool,
    stop_hit: u64,
    max_steps: u64,
    hold_a: bool,
    hold_start: bool,
    input_script: Vec<InputEvent>,
}

#[derive(Clone)]
struct InputEvent {
    start: u64,
    duration: u64,
    buttons: Vec<Button>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    let mut gba = if let Some(path) = args.load_state.as_deref() {
        Gba::load_state(path)?
    } else {
        let rom = load_rom_file(&args.rom)?;
        let save_path = args
            .save
            .clone()
            .unwrap_or_else(|| default_save_path(&args.rom));
        Gba::new(rom, Some(save_path), args.trace)
    };
    if args.trace {
        gba.set_trace(true);
    }
    if args.hold_a {
        gba.set_button(Button::A, true);
    }
    if args.hold_start {
        gba.set_button(Button::Start, true);
    }

    if let Some(stop_pc) = args.stop_pc {
        let hit = gba.run_until_pc_hit(stop_pc, args.stop_hit, args.max_steps);
        println!("stop_pc={stop_pc:08x} stop_hit={} hit={hit}", args.stop_hit);
        if args.dump_state {
            println!("{}", gba.debug_summary());
        }
        save_state_if_requested(&gba, &args)?;
        gba.flush_save()?;
        return Ok(());
    }

    if args.stop_invalid {
        let hit = gba.run_until_invalid(args.max_steps);
        println!("stop_invalid hit={hit}");
        if args.dump_state {
            println!("{}", gba.debug_summary());
        }
        save_state_if_requested(&gba, &args)?;
        gba.flush_save()?;
        return Ok(());
    }

    if let Some(frames) = args.frames {
        for frame in 0..frames {
            apply_headless_input(&mut gba, &args, frame);
            gba.run_frame();
        }
        if let Some(path) = args.screenshot.as_deref() {
            write_png(path, gba.framebuffer())?;
        }
        if args.dump_state {
            println!("{}", gba.debug_summary());
        }
        save_state_if_requested(&gba, &args)?;
        gba.flush_save()?;
        return Ok(());
    }

    let mut window = Window::new(
        "vibe-gba",
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        WindowOptions {
            scale: Scale::X4,
            resize: false,
            ..WindowOptions::default()
        },
    )?;
    window.set_target_fps(0);

    let frame_time = Duration::from_secs_f64(1.0 / 59.7275);
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let start = Instant::now();
        poll_input(&window, &mut gba);
        gba.run_frame();
        window.update_with_buffer(gba.framebuffer(), SCREEN_WIDTH, SCREEN_HEIGHT)?;
        gba.flush_save_if_dirty()?;

        if !args.turbo {
            let elapsed = start.elapsed();
            if elapsed < frame_time {
                std::thread::sleep(frame_time - elapsed);
            }
        }
    }

    save_state_if_requested(&gba, &args)?;
    gba.flush_save()?;
    Ok(())
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut rom = None;
    let mut save = None;
    let mut screenshot = None;
    let mut save_state = None;
    let mut load_state = None;
    let mut frames = None;
    let mut turbo = false;
    let mut trace = false;
    let mut dump_state = false;
    let mut stop_pc = None;
    let mut stop_invalid = false;
    let mut stop_hit = 1;
    let mut max_steps = 50_000_000;
    let mut hold_a = false;
    let mut hold_start = false;
    let mut input_script = Vec::new();

    let mut iter = std::env::args_os().skip(1);
    while let Some(arg) = iter.next() {
        let text = arg.to_string_lossy();
        match text.as_ref() {
            "--save" => save = iter.next().map(PathBuf::from),
            "--screenshot" => screenshot = iter.next().map(PathBuf::from),
            "--save-state" => save_state = iter.next().map(PathBuf::from),
            "--load-state" => load_state = iter.next().map(PathBuf::from),
            "--frames" => {
                let value = iter
                    .next()
                    .ok_or("--frames needs a number")?
                    .to_string_lossy()
                    .parse::<u64>()?;
                frames = Some(value);
            }
            "--turbo" => turbo = true,
            "--trace" => trace = true,
            "--dump-state" => dump_state = true,
            "--hold-a" => hold_a = true,
            "--hold-start" => hold_start = true,
            "--input-script" => {
                let value = iter.next().ok_or("--input-script needs a script")?;
                input_script = parse_input_script(&value.to_string_lossy())?;
            }
            "--stop-pc" => {
                let value = iter.next().ok_or("--stop-pc needs an address")?;
                stop_pc = Some(parse_u32(&value.to_string_lossy())?);
            }
            "--stop-invalid" => stop_invalid = true,
            "--stop-hit" => {
                stop_hit = iter
                    .next()
                    .ok_or("--stop-hit needs a number")?
                    .to_string_lossy()
                    .parse::<u64>()?;
            }
            "--max-steps" => {
                max_steps = iter
                    .next()
                    .ok_or("--max-steps needs a number")?
                    .to_string_lossy()
                    .parse::<u64>()?;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ if text.starts_with("--") => return Err(format!("unknown flag: {text}").into()),
            _ => {
                if rom.replace(PathBuf::from(arg)).is_some() {
                    return Err("only one ROM path is supported".into());
                }
            }
        }
    }

    let rom = rom.ok_or("usage: vibe-gba <rom.gba|zip> [--frames N] [--screenshot out.png]")?;
    Ok(Args {
        rom,
        save,
        screenshot,
        save_state,
        load_state,
        frames,
        turbo,
        trace,
        dump_state,
        stop_pc,
        stop_invalid,
        stop_hit,
        max_steps,
        hold_a,
        hold_start,
        input_script,
    })
}

fn print_help() {
    eprintln!(
        "usage: vibe-gba <rom.gba|zip> [--save file.sav] [--frames N] [--screenshot out.png] [--save-state state.bin] [--load-state state.bin] [--dump-state] [--hold-a] [--hold-start] [--input-script frame:duration:buttons,...] [--stop-pc HEX] [--stop-hit N] [--stop-invalid] [--max-steps N] [--turbo] [--trace]"
    );
}

fn save_state_if_requested(gba: &Gba, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = args.save_state.as_deref() {
        gba.save_state(path)?;
    }
    Ok(())
}

fn parse_u32(text: &str) -> Result<u32, Box<dyn std::error::Error>> {
    let trimmed = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    Ok(u32::from_str_radix(trimmed, 16)?)
}

fn poll_input(window: &Window, gba: &mut Gba) {
    gba.set_button(
        Button::A,
        window.is_key_down(Key::Z) || window.is_key_down(Key::A),
    );
    gba.set_button(
        Button::B,
        window.is_key_down(Key::X) || window.is_key_down(Key::S),
    );
    gba.set_button(Button::Select, window.is_key_down(Key::Backspace));
    gba.set_button(Button::Start, window.is_key_down(Key::Enter));
    gba.set_button(Button::Right, window.is_key_down(Key::Right));
    gba.set_button(Button::Left, window.is_key_down(Key::Left));
    gba.set_button(Button::Up, window.is_key_down(Key::Up));
    gba.set_button(Button::Down, window.is_key_down(Key::Down));
    gba.set_button(Button::R, window.is_key_down(Key::E));
    gba.set_button(Button::L, window.is_key_down(Key::Q));
}

fn apply_headless_input(gba: &mut Gba, args: &Args, frame: u64) {
    let mut pressed = [false; 10];
    pressed[Button::A as usize] = args.hold_a;
    pressed[Button::Start as usize] = args.hold_start;
    for event in &args.input_script {
        if frame >= event.start && frame < event.start.saturating_add(event.duration) {
            for &button in &event.buttons {
                pressed[button as usize] = true;
            }
        }
    }
    for (idx, &is_pressed) in pressed.iter().enumerate() {
        gba.set_button(button_from_index(idx), is_pressed);
    }
}

fn button_from_index(idx: usize) -> Button {
    match idx {
        0 => Button::A,
        1 => Button::B,
        2 => Button::Select,
        3 => Button::Start,
        4 => Button::Right,
        5 => Button::Left,
        6 => Button::Up,
        7 => Button::Down,
        8 => Button::R,
        9 => Button::L,
        _ => unreachable!(),
    }
}

fn parse_input_script(text: &str) -> Result<Vec<InputEvent>, Box<dyn std::error::Error>> {
    let mut events = Vec::new();
    if text.trim().is_empty() {
        return Ok(events);
    }
    for raw_event in text.split(',') {
        let mut parts = raw_event.split(':');
        let start = parts
            .next()
            .ok_or("input event needs a start frame")?
            .parse::<u64>()?;
        let duration = parts
            .next()
            .ok_or("input event needs a duration")?
            .parse::<u64>()?;
        let buttons = parts
            .next()
            .ok_or("input event needs buttons")?
            .split('+')
            .map(parse_button)
            .collect::<Result<Vec<_>, _>>()?;
        if parts.next().is_some() {
            return Err(format!("too many ':' separators in input event: {raw_event}").into());
        }
        events.push(InputEvent {
            start,
            duration,
            buttons,
        });
    }
    Ok(events)
}

fn parse_button(text: &str) -> Result<Button, Box<dyn std::error::Error>> {
    match text.trim().to_ascii_lowercase().as_str() {
        "a" => Ok(Button::A),
        "b" => Ok(Button::B),
        "select" => Ok(Button::Select),
        "start" => Ok(Button::Start),
        "right" => Ok(Button::Right),
        "left" => Ok(Button::Left),
        "up" => Ok(Button::Up),
        "down" => Ok(Button::Down),
        "r" => Ok(Button::R),
        "l" => Ok(Button::L),
        _ => Err(format!("unknown button: {text}").into()),
    }
}

fn default_save_path(rom: &Path) -> PathBuf {
    let mut path = rom.to_path_buf();
    path.set_extension("sav");
    path
}

fn write_png(path: &Path, frame: &[u32]) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::create(path)?;
    let writer = std::io::BufWriter::new(file);
    let mut encoder = png::Encoder::new(writer, SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let mut bytes = Vec::with_capacity(SCREEN_WIDTH * SCREEN_HEIGHT * 4);
    for &pixel in frame {
        bytes.push(((pixel >> 16) & 0xff) as u8);
        bytes.push(((pixel >> 8) & 0xff) as u8);
        bytes.push((pixel & 0xff) as u8);
        bytes.push(0xff);
    }
    writer.write_image_data(&bytes)?;
    Ok(())
}
