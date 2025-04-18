use chrono::{DateTime, Datelike, Utc};
use core::fmt::Write;
use esp_mbedtls::Tls;

use embedded_graphics::{
    image::Image,
    mono_font::MonoTextStyle,
    prelude::*,
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
};

use defmt::info;
use embassy_net::Stack;
use embassy_time::{Delay, Duration, Timer};
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyleBuilder},
    prelude::Point,
    text::{Baseline, Text},
};
use embedded_hal_bus::spi::ExclusiveDevice;
use epd_waveshare::{
    color::Color,
    epd1in54::Display1in54,
    epd1in54_v2::{self, Epd1in54},
    prelude::{DisplayRotation, WaveshareDisplay},
};
use esp_hal::{
    gpio::{Input, Output},
    peripherals::{RSA, SHA},
    rng::Rng,
    spi::master::Spi,
};
use heapless::String;
use profont::{PROFONT_18_POINT, PROFONT_24_POINT};
use reqwless::TlsReference;
use tinybmp::Bmp;

use crate::{icons::ICONS, weather::WeatherApi};

type SpiDevice = ExclusiveDevice<Spi<'static, esp_hal::Blocking>, Output<'static>, Delay>;
type EPD = Epd1in54<SpiDevice, Input<'static>, Output<'static>, Output<'static>, Delay>;

pub struct Dashboard {
    display: Display1in54,
    wifi: Stack<'static>,
    _rng: Rng,
    epd: EPD,
    spi_dev: SpiDevice,
}

impl Dashboard {
    pub fn new(wifi: Stack<'static>, rng: Rng, epd: EPD, spi_dev: SpiDevice) -> Self {
        Self {
            display: Display1in54::default(),
            wifi,
            _rng: rng,
            epd,
            spi_dev,
        }
    }

    pub async fn start(&mut self, sha: SHA, rsa: RSA) {
        self.display.set_rotation(DisplayRotation::Rotate90);

        let tls = Tls::new(sha)
            .expect("TLS::new with peripherals.SHA failed")
            .with_hardware_rsa(rsa);

        let api = WeatherApi::new(self.wifi);
        loop {
            self.refresh(&api, tls.reference()).await;

            Timer::after(Duration::from_secs(60)).await;
        }
    }

    pub async fn refresh(&mut self, api: &WeatherApi, tls_reference: TlsReference<'_>) {
        info!("Getting weather data");
        let weather_data = api.access_website(tls_reference).await;
        info!("Got weather data");

        self.epd.wake_up(&mut self.spi_dev, &mut Delay).unwrap();
        Timer::after(Duration::from_secs(5)).await;

        // Clear any existing image
        self.epd.clear_frame(&mut self.spi_dev, &mut Delay).unwrap();
        self.display.clear(Color::White).unwrap();
        self.epd
            .update_and_display_frame(&mut self.spi_dev, self.display.buffer(), &mut Delay)
            .unwrap();
        Timer::after(Duration::from_secs(5)).await;

        self.draw_date(weather_data.dt);

        self.draw_icon(weather_data.weather[0].id.icon(), Point::new(20, 50));
        self.draw_temperature(weather_data.main.temp, Point::new(20 + 70, 60));

        self.draw_humidity(weather_data.main.humidity);
        self.draw_wind(weather_data.wind.speed);

        self.draw_signature();

        self.epd
            .update_and_display_frame(&mut self.spi_dev, self.display.buffer(), &mut Delay)
            .unwrap();
        Timer::after(Duration::from_secs(5)).await;

        self.epd.sleep(&mut self.spi_dev, &mut Delay).unwrap();
    }

    pub fn get_icon(&self, icon_name: &'static str) -> Option<&'static [u8]> {
        ICONS
            .iter()
            .find(|(name, _)| *name == icon_name)
            .map(|(_, img_bytes)| *img_bytes)
    }

    fn draw_icon(&mut self, icon_name: &'static str, pos: Point) {
        let img_bytes = self.get_icon(icon_name).unwrap();

        let bmp = Bmp::from_slice(img_bytes).unwrap();
        let image = Image::new(&bmp, pos);
        image.draw(&mut self.display).unwrap();
    }

    fn draw_temperature(&mut self, temperature: f64, pos: Point) {
        let text_style = MonoTextStyle::new(&PROFONT_24_POINT, Color::Black);

        info!("Drawing temperature");
        let mut text: String<20> = String::new();
        write!(&mut text, "{}°C", temperature).unwrap();

        Text::with_baseline(&text, pos, text_style, Baseline::Top)
            .draw(&mut self.display)
            .unwrap();

        Line::new(Point::new(0, 105), Point::new(200, 105))
            .into_styled(PrimitiveStyle::with_stroke(Color::Black, 5))
            .draw(&mut self.display)
            .unwrap();
    }

    fn draw_humidity(&mut self, humidity: i32) {
        self.draw_icon("humidity_percentage.bmp", Point::new(5, 110));

        let text_style = MonoTextStyle::new(&PROFONT_18_POINT, Color::Black);

        let mut text: String<10> = String::new();
        write!(&mut text, "{}", humidity).unwrap();

        Text::with_baseline(&text, Point::new(5 + 50, 120), text_style, Baseline::Top)
            .draw(&mut self.display)
            .unwrap();

        Line::new(Point::new(5 + 85, 120), Point::new(5 + 85, 120 + 30))
            .into_styled(PrimitiveStyle::with_stroke(Color::Black, 5))
            .draw(&mut self.display)
            .unwrap();
    }

    fn draw_wind(&mut self, wind_speed: f64) {
        self.draw_icon("air.bmp", Point::new(100, 110));

        let text_style = MonoTextStyle::new(&PROFONT_18_POINT, Color::Black);

        let mut text: String<10> = String::new();
        write!(&mut text, "{}", wind_speed).unwrap();

        Text::with_baseline(&text, Point::new(100 + 50, 120), text_style, Baseline::Top)
            .draw(&mut self.display)
            .unwrap();

        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_10X20)
            .text_color(Color::Black)
            .build();

        Text::with_baseline("m/s", Point::new(100 + 50, 140), text_style, Baseline::Top)
            .draw(&mut self.display)
            .unwrap();
    }

    fn draw_date(&mut self, dt: DateTime<Utc>) {
        let text_style = MonoTextStyle::new(&PROFONT_24_POINT, Color::Black);

        let mut text: String<24> = String::new();
        write!(
            &mut text,
            "{} {} {}",
            dt.day(),
            month_name(dt.month()),
            dt.year()
        )
        .unwrap();

        Text::with_baseline(&text, Point::new(20, 10), text_style, Baseline::Top)
            .draw(&mut self.display)
            .unwrap();

        Line::new(Point::new(0, 45), Point::new(200, 45))
            .into_styled(PrimitiveStyle::with_stroke(Color::Black, 5))
            .draw(&mut self.display)
            .unwrap();
    }

    fn draw_signature(&mut self) {
        let display_width = epd1in54_v2::WIDTH as i32;
        let rect_padding = 20;

        let rect_width = display_width - 2 * rect_padding;
        let rect_height = 40;
        let rect_x = rect_padding;
        let rect_y = 170;

        let style = PrimitiveStyleBuilder::new()
            .stroke_color(Color::Black)
            .stroke_width(3)
            .fill_color(Color::Black)
            .build();

        Rectangle::new(
            Point::new(rect_x, rect_y),
            Size::new(rect_width as u32, rect_height as u32),
        )
        .into_styled(style)
        .draw(&mut self.display)
        .unwrap();

        let text = "implRust";
        let text_style = MonoTextStyle::new(&PROFONT_24_POINT, Color::White);

        let char_width = PROFONT_24_POINT.character_size.width as i32;
        let text_width = text.len() as i32 * char_width;
        let text_x = rect_x + (rect_width - text_width) / 2;

        Text::with_baseline(
            text,
            Point::new(text_x as i32, rect_y),
            text_style,
            Baseline::Top,
        )
        .draw(&mut self.display)
        .unwrap();
    }
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "Err",
    }
}
