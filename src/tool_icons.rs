use gtk::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Glyph {
    Package,
    AppDrive,
    Cache,
    Upgrade,
    Check,
    List,
    Search,
    History,
    Drive,
    Monitor,
    Mount,
    Pie,
    Services,
    Timer,
    Warning,
    Failed,
    Chain,
    Trim,
    Network,
    Dns,
    Power,
    Trash,
    Gpu,
}
pub(crate) fn for_tool(name: &str) -> Option<Glyph> {
    Some(match name {
        "Package cache location" => Glyph::Cache,
        "App install locations" => Glyph::AppDrive,
        "Full system upgrade" => Glyph::Upgrade,
        "Package file check" => Glyph::Check,
        "Explicit package list" => Glyph::List,
        "Find a file's package" => Glyph::Search,
        "Recent package transactions" => Glyph::History,
        "Drives and capacity" => Glyph::Drive,
        "Mounted filesystems" => Glyph::Mount,
        "Filesystem free space" => Glyph::Pie,
        "Enabled services" => Glyph::Services,
        "Scheduled timers" => Glyph::Timer,
        "Current boot errors" => Glyph::Warning,
        "Boot critical chain" => Glyph::Chain,
        "SSD trim schedule" => Glyph::Trim,
        "Network addresses" => Glyph::Network,
        "DNS resolver status" => Glyph::Dns,
        "Power profiles" => Glyph::Power,
        "Review old package archives" => Glyph::Trash,
        "Inspect failed services" => Glyph::Failed,
        "Inspect Vulkan support" => Glyph::Gpu,
        _ => return None,
    })
}
pub(crate) fn icon(glyph: Glyph, size: i32) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(size);
    area.set_content_height(size);
    area.set_halign(gtk::Align::Center);
    area.set_valign(gtk::Align::Center);
    area.set_accessible_role(gtk::AccessibleRole::Presentation);
    area.set_draw_func(move |area, c, w, h| {
        let color = area.color();
        c.set_source_rgba(
            color.red() as f64,
            color.green() as f64,
            color.blue() as f64,
            color.alpha() as f64,
        );
        c.scale(w as f64 / 24., h as f64 / 24.);
        c.set_line_width(1.65);
        c.set_line_cap(gtk::cairo::LineCap::Round);
        c.set_line_join(gtk::cairo::LineJoin::Round);
        let line = |points: &[(f64, f64)]| {
            if let Some((x, y)) = points.first() {
                c.move_to(*x, *y);
                for (x, y) in &points[1..] {
                    c.line_to(*x, *y);
                }
                let _ = c.stroke();
            }
        };
        let rect = |x, y, w, h| {
            c.rectangle(x, y, w, h);
            let _ = c.stroke();
        };
        let circle = |x, y, r| {
            c.new_sub_path();
            c.arc(x, y, r, 0., std::f64::consts::TAU);
            let _ = c.stroke();
        };
        match glyph {
            Glyph::Package => {
                line(&[
                    (12., 2.),
                    (21., 7.),
                    (21., 17.),
                    (12., 22.),
                    (3., 17.),
                    (3., 7.),
                    (12., 2.),
                ]);
                line(&[(3., 7.), (12., 12.), (21., 7.)]);
                line(&[(12., 12.), (12., 22.)]);
                line(&[(7.5, 4.5), (16.5, 9.5), (16.5, 13.)]);
            }
            Glyph::AppDrive => {
                rect(3., 3., 18., 18.);
                line(&[(3., 9.), (21., 9.)]);
                circle(6., 6., 0.5);
                line(&[(12., 11.), (12., 19.)]);
                line(&[(8., 15.), (12., 19.), (16., 15.)]);
            }
            Glyph::Cache => {
                line(&[
                    (4., 8.),
                    (2.5, 18.),
                    (5., 21.),
                    (19., 21.),
                    (21.5, 18.),
                    (20., 8.),
                    (16., 8.),
                ]);
                line(&[(8., 8.), (4., 8.)]);
                line(&[(12., 2.), (12., 14.)]);
                line(&[(8., 10.), (12., 14.), (16., 10.)]);
            }
            Glyph::Upgrade => {
                line(&[(12., 17.), (12., 3.)]);
                line(&[(7., 8.), (12., 3.), (17., 8.)]);
                line(&[(4., 16.), (4., 21.), (20., 21.), (20., 16.)]);
            }
            Glyph::Check => {
                line(&[
                    (15., 2.),
                    (5., 2.),
                    (5., 22.),
                    (20., 22.),
                    (20., 7.),
                    (15., 2.),
                    (15., 7.),
                    (20., 7.),
                ]);
                line(&[(8., 14.), (11., 17.), (16., 12.)]);
            }
            Glyph::List => {
                for y in [5., 12., 19.] {
                    circle(4., y, 1.);
                    line(&[(9., y), (21., y)]);
                }
            }
            Glyph::Search => {
                circle(10., 10., 7.);
                line(&[(15.5, 15.5), (22., 22.)]);
            }
            Glyph::History => {
                circle(12., 12., 9.);
                line(&[(12., 6.), (12., 12.), (16., 14.)]);
            }
            Glyph::Drive => {
                line(&[
                    (6., 3.),
                    (18., 3.),
                    (21., 18.),
                    (21., 21.),
                    (3., 21.),
                    (3., 18.),
                    (6., 3.),
                ]);
                line(&[(3., 16.), (21., 16.)]);
                circle(17., 18.5, 0.6);
            }
            Glyph::Monitor => {
                rect(3., 4., 18., 13.);
                line(&[(8., 21.), (16., 21.)]);
                line(&[(12., 17.), (12., 21.)]);
            }
            Glyph::Mount => {
                rect(3., 3., 18., 7.);
                rect(3., 14., 18., 7.);
                circle(6., 6.5, 0.6);
                circle(6., 17.5, 0.6);
                line(&[(16., 6.5), (18., 6.5)]);
                line(&[(16., 17.5), (18., 17.5)]);
            }
            Glyph::Pie => {
                c.arc(11., 13., 9., -std::f64::consts::FRAC_PI_2, 0.);
                let _ = c.stroke();
                line(&[(11., 4.), (11., 13.), (20., 13.)]);
                c.arc(11., 13., 9., 0., std::f64::consts::PI * 1.5);
                let _ = c.stroke();
                line(&[(15., 2.), (15., 9.), (22., 9.)]);
            }
            Glyph::Services => {
                rect(5., 5., 14., 14.);
                circle(12., 12., 3.);
                for t in [8., 16.] {
                    line(&[(t, 2.), (t, 5.)]);
                    line(&[(t, 19.), (t, 22.)]);
                    line(&[(2., t), (5., t)]);
                    line(&[(19., t), (22., t)]);
                }
            }
            Glyph::Timer => {
                circle(12., 14., 8.);
                line(&[(9., 2.), (15., 2.)]);
                line(&[(12., 2.), (12., 6.)]);
                line(&[(18., 6.), (20., 4.)]);
                line(&[(12., 9.), (12., 14.), (16., 14.)]);
            }
            Glyph::Warning => {
                line(&[(12., 2.), (23., 21.), (1., 21.), (12., 2.)]);
                line(&[(12., 8.), (12., 14.)]);
                circle(12., 18., 0.5);
            }
            Glyph::Chain => {
                rect(2., 3., 8., 6.);
                rect(14., 15., 8., 6.);
                line(&[(6., 9.), (6., 18.), (14., 18.)]);
                line(&[(10., 6.), (18., 6.), (18., 15.)]);
            }
            Glyph::Trim => {
                rect(3., 3., 18., 15.);
                line(&[(7., 21.), (17., 21.)]);
                line(&[(7., 10.), (10., 13.), (17., 6.)]);
            }
            Glyph::Network => {
                rect(7., 2., 10., 7.);
                rect(2., 17., 7., 5.);
                rect(15., 17., 7., 5.);
                line(&[(12., 9.), (12., 13.), (5.5, 13.), (5.5, 17.)]);
                line(&[(12., 13.), (18.5, 13.), (18.5, 17.)]);
            }
            Glyph::Dns => {
                circle(9., 9., 7.);
                line(&[(2., 9.), (16., 9.)]);
                line(&[(9., 2.), (9., 16.)]);
                circle(17., 17., 4.);
                line(&[(20., 20.), (23., 23.)]);
            }
            Glyph::Failed => {
                rect(3., 3., 18., 7.);
                rect(3., 14., 18., 7.);
                line(&[(6., 5.), (10., 8.)]);
                line(&[(10., 5.), (6., 8.)]);
                circle(7., 17.5, 0.6);
                line(&[(15., 17.5), (18., 17.5)]);
            }
            Glyph::Power => {
                rect(2., 6., 18., 12.);
                line(&[(22., 10.), (22., 14.)]);
                line(&[(12., 8.), (8., 12.), (13., 12.), (9., 16.)]);
            }
            Glyph::Trash => {
                line(&[(3., 5.), (21., 5.)]);
                line(&[(9., 5.), (9., 2.), (15., 2.), (15., 5.)]);
                line(&[(5., 5.), (6., 22.), (18., 22.), (19., 5.)]);
                line(&[(10., 9.), (10., 18.)]);
                line(&[(14., 9.), (14., 18.)]);
            }
            Glyph::Gpu => {
                rect(3., 5., 18., 14.);
                circle(12., 12., 4.);
                line(&[(1., 9.), (3., 9.)]);
                line(&[(1., 15.), (3., 15.)]);
                for x in [7., 11., 15., 19.] {
                    line(&[(x, 19.), (x, 22.)]);
                }
            }
        }
    });
    area
}
