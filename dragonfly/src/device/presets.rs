//! The devices a Dragonfly device can be: the sizes of the browsers' device toolbar (CSS pixels, portrait).
//! The same list as the VS Code device panel (editors/vscode/src/device.ts).

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Os {
    Ios,
    Android,
    Web,
}

#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub name: &'static str,
    /// short name for the command line: `dragonfly device pixel7`
    pub id: &'static str,
    pub width: u32,
    pub height: u32,
    pub dpr: f64,
    pub os: Os,
    pub tablet: bool,
}

const fn p(name: &'static str, id: &'static str, width: u32, height: u32, dpr: f64, os: Os, tablet: bool) -> Preset {
    Preset { name, id, width, height, dpr, os, tablet }
}

pub const PRESETS: &[Preset] = &[
    p("Pixel 7", "pixel7", 412, 915, 2.625, Os::Android, false),
    p("iPhone 14 Pro", "iphone14pro", 393, 852, 3.0, Os::Ios, false),
    p("iPhone SE", "iphonese", 375, 667, 2.0, Os::Ios, false),
    p("iPhone 15 Pro Max", "iphone15promax", 430, 932, 3.0, Os::Ios, false),
    p("Samsung Galaxy S20 Ultra", "galaxys20", 412, 915, 3.5, Os::Android, false),
    p("Samsung Galaxy A51/71", "galaxya51", 412, 914, 2.625, Os::Android, false),
    p("iPad Mini", "ipadmini", 768, 1024, 2.0, Os::Ios, true),
    p("iPad Air", "ipadair", 820, 1180, 2.0, Os::Ios, true),
    p("iPad Pro", "ipadpro", 1024, 1366, 2.0, Os::Ios, true),
    p("Galaxy Tab S7", "tabs7", 800, 1280, 2.0, Os::Android, true),
    p("Responsive", "responsive", 400, 800, 1.0, Os::Web, false),
    p("Laptop", "laptop", 1280, 800, 1.0, Os::Web, false),
    p("Desktop", "desktop", 1920, 1080, 1.0, Os::Web, false),
];

const IOS_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";
const IPAD_UA: &str = "Mozilla/5.0 (iPad; CPU OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";
const ANDROID_UA: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Mobile Safari/537.36";

impl Preset {
    /// A preset by its id or name, ignoring case, spaces and dashes (`pixel7`, `"Pixel 7"`, `iphone-se`).
    pub fn find(query: &str) -> Option<Preset> {
        let norm = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
        let q = norm(query);
        PRESETS.iter().find(|p| p.id == q || norm(p.name) == q).copied()
    }

    pub fn mobile(&self) -> bool {
        self.os != Os::Web
    }

    /// The browser identity: user agent and `navigator.platform` (none for web: the browser's own).
    pub fn identity(&self) -> Option<(&'static str, &'static str)> {
        match (self.os, self.tablet) {
            (Os::Web, _) => None,
            (Os::Android, _) => Some((ANDROID_UA, "Linux armv8l")),
            (Os::Ios, true) => Some((IPAD_UA, "iPad")),
            (Os::Ios, false) => Some((IOS_UA, "iPhone")),
        }
    }

    /// CSS size with the rotation applied.
    pub fn size(&self, landscape: bool) -> (u32, u32) {
        if landscape { (self.height, self.width) } else { (self.width, self.height) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_found_by_id_or_name() {
        assert_eq!(Preset::find("pixel7").unwrap().name, "Pixel 7");
        assert_eq!(Preset::find("iPhone SE").unwrap().id, "iphonese");
        assert_eq!(Preset::find("iphone-14-pro").unwrap().width, 393);
        assert!(Preset::find("nokia").is_none());
        assert!(PRESETS.iter().all(|p| Preset::find(p.id).is_some()));
    }
}
