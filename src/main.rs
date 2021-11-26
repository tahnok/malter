use linux_embedded_hal::{
    spidev::{self, SpidevOptions},
    sysfs_gpio::Direction,
    Delay, Pin, Spidev,
};

use embedded_graphics::{
    fonts::{Font12x16, Font8x16},
    pixelcolor::BinaryColor::On as Black,
    prelude::*,
    primitives::Rectangle,
    style::PrimitiveStyleBuilder,
};

use epd_waveshare::{
    epd2in9::*,
    graphics::{Display, DisplayRotation},
    prelude::*,
};

use embedded_text::{alignment::center::CenterAligned, prelude::*};

struct IndoorData {
    temp: f32,
    hummidity: f32,
    pressure: f32,
}

fn draw(display: &mut Display2in9, indoor_data: &IndoorData) -> Result<(), std::io::Error> {
    let big_text_style = TextBoxStyleBuilder::new(Font12x16)
        .text_color(Black)
        .alignment(CenterAligned)
        .vertical_alignment(CenterAligned)
        .build();

    let small_text_style = TextBoxStyleBuilder::new(Font8x16)
        .text_color(Black)
        .alignment(CenterAligned)
        .vertical_alignment(CenterAligned)
        .build();

    let line_style = PrimitiveStyleBuilder::new()
        .stroke_color(Black)
        .stroke_width(1)
        .build();

    let left_top = Rectangle::new(
        Point::new(0, 0),
        Point::new(HEIGHT as i32 / 3, WIDTH as i32 / 2),
    );
    let left_bottom = Rectangle::new(
        Point::new(0, WIDTH as i32 / 2),
        Point::new(HEIGHT as i32 / 3, WIDTH as i32),
    );
    let _middle = Rectangle::new(
        Point::new(HEIGHT as i32 / 3, 0),
        Point::new((HEIGHT as i32 / 3) * 2, WIDTH as i32),
    );
    let _right = Rectangle::new(
        Point::new((HEIGHT as i32 / 3) * 2, 0),
        Point::new(HEIGHT as i32, WIDTH as i32),
    );

    let temp_txt = format!("{:.1}C", indoor_data.temp);
    let text_box1 = TextBox::new(&temp_txt, left_top).into_styled(big_text_style);
    text_box1.draw(display).expect("impossible");

    left_top
        .into_styled(line_style)
        .draw(display)
        .expect("impossible");

    let minor_text = format!("{:.1}%\n{:.0} hPa", indoor_data.hummidity, indoor_data.pressure);
    let text_box2 = TextBox::new(&minor_text, left_bottom).into_styled(small_text_style);
    text_box2.draw(display).expect("impossible");

    left_bottom
        .into_styled(line_style)
        .draw(display)
        .expect("impossible");

    Ok(())
}

fn main() -> Result<(), std::io::Error> {
    let (mut epd, mut spi) = get_epd()?;

    // Use display graphics from embedded-graphics
    let mut display = Display2in9::default();
    display.set_rotation(DisplayRotation::Rotate90);

    let data = IndoorData {
        temp: 21.4,
        hummidity: 49.2,
        pressure: 1026.1,
    };

    draw(&mut display, &data)?;

    // Display updated frame
    epd.update_and_display_frame(&mut spi, &display.buffer())?;

    // Set the EPD to sleep
    epd.sleep(&mut spi)?;

    Ok(())
}


fn get_epd() -> Result<(EPD2in9<Spidev, Pin, Pin, Pin, Pin>, Spidev), std::io::Error> {
    // Configure SPI
    // Settings are taken from
    let mut spi = Spidev::open("/dev/spidev0.0").expect("spidev directory");
    let options = SpidevOptions::new()
        .bits_per_word(8)
        .max_speed_hz(4_000_000)
        .mode(spidev::SpiModeFlags::SPI_MODE_0)
        .build();
    spi.configure(&options).expect("spi configuration");

    // Configure Digital I/O Pin to be used as Chip Select for SPI
    let cs = Pin::new(24); //BCM7 CE0
    cs.export().expect("cs export");
    while !cs.is_exported() {}
    cs.set_direction(Direction::Out).expect("CS Direction");
    cs.set_value(1).expect("CS Value set to 1");

    let busy = Pin::new(24); //pin 18
    busy.export().expect("busy export");
    while !busy.is_exported() {}
    busy.set_direction(Direction::In).expect("busy Direction");
    //busy.set_value(1).expect("busy Value set to 1");

    let dc = Pin::new(25); //pin 22
    dc.export().expect("dc export");
    while !dc.is_exported() {}
    dc.set_direction(Direction::Out).expect("dc Direction");
    dc.set_value(1).expect("dc Value set to 1");

    let rst = Pin::new(17); //pin 11
    rst.export().expect("rst export");
    while !rst.is_exported() {}
    rst.set_direction(Direction::Out).expect("rst Direction");
    rst.set_value(1).expect("rst Value set to 1");

    let mut delay = Delay {};

    // Setup EPD
    let epd = EPD2in9::new(&mut spi, cs, busy, dc, rst, &mut delay)?;
    return Ok((epd, spi));
}