//! Original line icons on a 24pt grid, matching Home's lightweight visual language.
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Stroke};
#[derive(Clone, Copy)]
pub enum Icon {
    Home,
    Automation,
    Star,
    Climate,
    Display,
    Usage,
    Room,
    Chevron,
    Plus,
    More,
    Settings,
    Bluetooth,
    Lock,
    Close,
    Info,
    Thermometer,
    Humidity,
    Refresh,
    Back,
}
pub fn draw(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let at = |x: f32, y: f32| {
        egui::pos2(
            rect.left() + x * rect.width() / 24.0,
            rect.top() + y * rect.height() / 24.0,
        )
    };
    let stroke = Stroke::new((rect.width() / 16.0).clamp(1.3, 2.4), color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([at(a.0, a.1), at(b.0, b.1)], stroke);
    };
    let path = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points.iter().map(|&(x, y)| at(x, y)).collect::<Vec<Pos2>>(),
            stroke,
        ));
    };
    let circle = |x: f32, y: f32, r: f32| {
        p.circle_stroke(at(x, y), r * rect.width() / 24.0, stroke);
    };
    match icon {
        Icon::Back => {
            path(&[(15., 5.), (8., 12.), (15., 19.)]);
        }
        Icon::Home => {
            path(&[(2., 11.), (12., 2.), (22., 11.)]);
            path(&[
                (5., 9.),
                (5., 22.),
                (10., 22.),
                (10., 15.),
                (15., 15.),
                (15., 22.),
                (20., 22.),
                (20., 9.),
            ]);
        }
        Icon::Automation => {
            circle(12., 11., 9.);
            path(&[(7., 10.), (11., 14.), (17., 7.)]);
            path(&[(4., 19.), (4., 22.), (20., 22.), (20., 19.)]);
        }
        Icon::Star => {
            path(&[
                (12., 2.),
                (15., 9.),
                (22., 10.),
                (17., 15.),
                (18., 22.),
                (12., 18.),
                (6., 22.),
                (7., 15.),
                (2., 10.),
                (9., 9.),
                (12., 2.),
            ]);
        }
        Icon::Display => {
            p.rect_stroke(
                Rect::from_min_max(at(2., 4.), at(22., 18.)),
                3,
                stroke,
                egui::StrokeKind::Inside,
            );
            line((12., 18.), (12., 22.));
            line((7., 22.), (17., 22.));
            path(&[(6., 9.), (9., 9.)]);
            line((6., 13.), (18., 13.));
            line((12., 9.), (18., 9.));
        }
        Icon::Climate => {
            circle(12., 12., 2.);
            for i in 0..4 {
                let angle = i as f32 * std::f32::consts::FRAC_PI_2;
                let center = at(12. + angle.cos() * 6., 12. + angle.sin() * 6.);
                p.circle_filled(center, 3.5 * rect.width() / 24., color);
            }
        }
        Icon::Thermometer => {
            path(&[
                (9., 14.),
                (9., 5.),
                (10., 3.),
                (13., 3.),
                (15., 5.),
                (15., 14.),
            ]);
            path(&[
                (9., 14.),
                (6., 17.),
                (7., 21.),
                (10., 23.),
                (14., 23.),
                (17., 21.),
                (18., 17.),
                (15., 14.),
            ]);
            line((12., 8.), (12., 18.));
            circle(12., 18., 1.5);
        }
        Icon::Humidity => {
            path(&[
                (12., 2.),
                (5., 11.),
                (3.5, 15.),
                (5., 20.),
                (9., 22.),
                (15., 22.),
                (19., 20.),
                (20.5, 15.),
                (19., 11.),
                (12., 2.),
            ]);
            path(&[(7., 15.), (8., 18.), (11., 19.)]);
        }
        Icon::Refresh => {
            path(&[
                (20., 9.),
                (18., 5.),
                (14., 3.),
                (9., 3.),
                (5., 6.),
                (3., 10.),
                (3., 14.),
                (5., 18.),
                (9., 21.),
                (14., 21.),
                (18., 18.),
            ]);
            path(&[(20., 3.), (20., 9.), (14., 9.)]);
        }
        Icon::Usage => {
            for (x, h) in [(5., 8.), (12., 16.), (19., 12.)] {
                line((x, 21.), (x, 21. - h));
            }
        }
        Icon::Room => {
            p.rect_stroke(
                Rect::from_min_max(at(3., 3.), at(21., 21.)),
                3,
                stroke,
                egui::StrokeKind::Inside,
            );
            path(&[(12., 21.), (12., 12.), (21., 12.)]);
        }
        Icon::Chevron => path(&[(9., 5.), (16., 12.), (9., 19.)]),
        Icon::Plus => {
            line((12., 3.), (12., 21.));
            line((3., 12.), (21., 12.));
        }
        Icon::More => {
            for x in [4., 12., 20.] {
                p.circle_filled(at(x, 12.), rect.width() / 13., color);
            }
        }
        Icon::Settings => {
            circle(12., 12., 4.);
            circle(12., 12., 9.);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                line(
                    (12. + a.cos() * 9., 12. + a.sin() * 9.),
                    (12. + a.cos() * 11., 12. + a.sin() * 11.),
                );
            }
        }
        Icon::Bluetooth => path(&[
            (6., 5.),
            (18., 17.),
            (12., 22.),
            (12., 2.),
            (18., 7.),
            (6., 19.),
        ]),
        Icon::Lock => {
            p.rect_stroke(
                Rect::from_min_max(at(5., 10.), at(19., 22.)),
                2,
                stroke,
                egui::StrokeKind::Inside,
            );
            path(&[
                (8., 10.),
                (8., 6.),
                (9., 3.),
                (15., 3.),
                (16., 6.),
                (16., 10.),
            ]);
        }
        Icon::Close => {
            line((5., 5.), (19., 19.));
            line((19., 5.), (5., 19.));
        }
        Icon::Info => {
            circle(12., 12., 10.);
            line((12., 10.), (12., 18.));
            p.circle_filled(at(12., 6.), rect.width() / 17., color);
        }
    }
}
