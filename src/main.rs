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

use ureq;

use std::{error, fmt, fs, result};

use serde::Deserialize;

use chrono::prelude::*;

#[derive(Deserialize)]
struct Config {
    influx_server: String,
    influx_database: String,
    lat: String,
    lon: String,
    openweather_api_key: String,
}

struct IndoorData {
    temp: f64,
    humidity: f64,
    pressure: f64,
}

#[derive(Debug)]
struct OutdoorData {
    temp: f64,
    humidity: f64,
    pressure: f64,
}

struct ForecastData {
    high: f64,
    low: f64,
    description: String,
    pop: f64,
}

#[derive(Debug)]
struct Oops(String);

impl From<std::io::Error> for Oops {
    fn from(e: std::io::Error) -> Oops {
        Oops(e.to_string())
    }
}

impl From<ureq::Error> for Oops {
    fn from(e: ureq::Error) -> Oops {
        Oops(e.to_string())
    }
}

impl From<toml::de::Error> for Oops {
    fn from(e: toml::de::Error) -> Oops {
        Oops(e.to_string())
    }
}

impl error::Error for Oops {}

impl fmt::Display for Oops {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

type Result<T> = result::Result<T, Oops>;

fn main() -> Result<()> {
    let local: DateTime<Local> = Local::now();
    let hour = local.hour();

    if hour > 22 || hour < 7 {
        println!("bed time, sleeping...");
        return Ok(());
    }

    let conf_file =
        fs::read_to_string("conf.toml").expect("Missing conf.toml, try copying conf-sample.toml");
    let config: Config = toml::from_str(&conf_file)?;

    let (mut epd, mut spi) = get_epd()?;

    // Use display graphics from embedded-graphics
    let mut display = Display2in9::default();
    display.set_rotation(DisplayRotation::Rotate90);

    let (indoor_data, outdoor_data, forecast_data) = get_data(&config)?;

    draw(&mut display, &indoor_data, &outdoor_data, &forecast_data)?;

    // Display updated frame
    epd.update_and_display_frame(&mut spi, &display.buffer())?;

    // Set the EPD to sleep
    epd.sleep(&mut spi)?;

    Ok(())
}

fn get_data(config: &Config) -> Result<(IndoorData, OutdoorData, ForecastData)> {
    let response: serde_json::Value = ureq::get(&config.influx_server)
        .query("pretty", "true")
        .query("db", &config.influx_database)
        .query("q", "SELECT MEAN(temperature) as temperature, MEAN(pressure) as pressure, MEAN(humidity) as humidity FROM \"indoor\" group by time(15m) order by time desc limit 1")
        .call()?
        .into_json()?;

    let values = &response["results"][0]["series"][0]["values"][0];

    let indoor_data = IndoorData {
        temp: values[1].as_f64().unwrap_or(0.0),
        humidity: values[3].as_f64().unwrap_or(0.0),
        pressure: values[2].as_f64().unwrap_or(0.0),
    };

    let response: serde_json::Value = ureq::get("https://api.openweathermap.org/data/2.5/onecall")
        .query("lat", &config.lat)
        .query("lon", &config.lon)
        .query("appid", &config.openweather_api_key)
        .query("units", "metric")
        .call()?
        .into_json()?;

    let outdoor_data = OutdoorData {
        temp: response["current"]["feels_like"].as_f64().unwrap_or(0.0),
        humidity: response["current"]["humidity"].as_f64().unwrap_or(0.0),
        pressure: response["current"]["pressure"].as_f64().unwrap_or(0.0),
    };

    let forecast_data = ForecastData {
        high: response["daily"][0]["temp"]["max"].as_f64().unwrap_or(0.0),
        low: response["daily"][0]["temp"]["min"].as_f64().unwrap_or(0.0),
        description: response["daily"][0]["weather"][0]["description"].to_string(),
        pop: response["daily"][0]["pop"].as_f64().unwrap_or(0.0),
    };


    return Ok((indoor_data, outdoor_data, forecast_data));
}

fn draw(
    display: &mut Display2in9,
    indoor_data: &IndoorData,
    outdoor_data: &OutdoorData,
    forecast_data: &ForecastData,
) -> Result<()> {
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

    // left column indoor data
    let left_top = Rectangle::new(
        Point::new(0, 0),
        Point::new(HEIGHT as i32 / 3, WIDTH as i32 / 2),
    );
    let left_bottom = Rectangle::new(
        Point::new(0, WIDTH as i32 / 2),
        Point::new(HEIGHT as i32 / 3, WIDTH as i32),
    );
    let temp_txt = format!("{:.1}C", indoor_data.temp);
    let text_box1 = TextBox::new(&temp_txt, left_top).into_styled(big_text_style);
    text_box1.draw(display).expect("impossible");

    left_top
        .into_styled(line_style)
        .draw(display)
        .expect("impossible");

    let minor_text = format!(
        "{:.1}%\n{:.0} hPa",
        indoor_data.humidity, indoor_data.pressure
    );
    let text_box2 = TextBox::new(&minor_text, left_bottom).into_styled(small_text_style);
    text_box2.draw(display).expect("impossible");

    left_bottom
        .into_styled(line_style)
        .draw(display)
        .expect("impossible");

    // middle outdoor temp
    let middle_top = Rectangle::new(
        Point::new(HEIGHT as i32 / 3, 0),
        Point::new((HEIGHT as i32 / 3) * 2, WIDTH as i32 / 2),
    );
    let middle_bottom = Rectangle::new(
        Point::new(HEIGHT as i32 / 3, WIDTH as i32 / 2),
        Point::new((HEIGHT as i32 / 3) * 2, WIDTH as i32),
    );

    let temp_txt = format!("{:.1}C", outdoor_data.temp);
    let text_box1 = TextBox::new(&temp_txt, middle_top).into_styled(big_text_style);
    text_box1.draw(display).expect("impossible");

    middle_top
        .into_styled(line_style)
        .draw(display)
        .expect("impossible");

    let minor_text = format!(
        "{:.1}%\n{:.0} hPa",
        outdoor_data.humidity, outdoor_data.pressure
    );
    let text_box2 = TextBox::new(&minor_text, middle_bottom).into_styled(small_text_style);
    text_box2.draw(display).expect("impossible");

    middle_bottom
        .into_styled(line_style)
        .draw(display)
        .expect("impossible");

    // right outdoor forecast
    let right = Rectangle::new(
        Point::new((HEIGHT as i32 / 3) * 2, 0),
        Point::new(HEIGHT as i32, WIDTH as i32),
    );

    let forecast_text = format!(
        "High: {:.1}\n  Low: {:.1}\n  Pop: {:.1}%\n\n{}",
        forecast_data.high,
        forecast_data.low,
        forecast_data.pop,
        forecast_data.description,
    );

    let text_box3 = TextBox::new(&forecast_text, right).into_styled(small_text_style);
    text_box3.draw(display).expect("impossible");

    right
        .into_styled(line_style)
        .draw(display)
        .expect("impossible");


    Ok(())
}

fn get_epd() -> Result<(EPD2in9<Spidev, Pin, Pin, Pin, Pin>, Spidev)> {
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
