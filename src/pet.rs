use egui::Color32;
use serde::{Deserialize, Serialize};

/// Which pet is sitting at the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum PetKind {
    /// Gato preto (o original)
    #[default]
    #[value(alias = "preto", alias = "gato")]
    BlackCat,
    /// Gato frajola (preto e branco)
    #[value(alias = "frajola")]
    Tuxedo,
    /// Gato bengal (dourado com pintas)
    Bengal,
    /// Gata tricolor (branco, laranja e preto)
    #[value(alias = "tricolor", alias = "malhado")]
    Calico,
    /// Gato siamês (creme com pontas escuras e olhos azuis)
    #[value(alias = "siames")]
    Siamese,
    /// Cachorro caramelo
    #[value(alias = "cachorro", alias = "caramelo")]
    Dog,
    /// Capivara
    #[value(alias = "capivara", hide = true)]
    Capybara,
    /// Dinossauro verdinho
    #[value(alias = "dinossauro", alias = "dinosauro", alias = "dinosaur", hide = true)]
    Dino,
    /// Alienígena de antenas
    #[value(alias = "alienigena", alias = "et", hide = true)]
    Alien,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Species {
    Cat,
    Dog,
    Capybara,
    Dino,
    Alien,
}

/// Colors and small anatomical switches used by the renderer for one pet.
pub struct PetStyle {
    pub species: Species,
    pub body: Color32,
    /// Contour lines drawn where a limb overlaps the torso.
    pub rim: Color32,
    /// Silhouette outline for very light coats, so they survive bright wallpapers.
    pub outline: Option<Color32>,
    /// Front paw ("sock") color.
    pub paw: Color32,
    /// Contour around light paws so they stay readable over white keys and the mouse.
    pub paw_outline: Option<Color32>,
    pub hind_paw: Color32,
    /// Ear colors: [viewer's left, viewer's right].
    pub ears: [Color32; 2],
    pub tail: Color32,
    pub tail_tip: Option<Color32>,
    pub eye: Color32,
    pub eye_ring: Color32,
    pub pupil: Color32,
    /// Closed / blissful eye line color per eye.
    pub eye_closed: [Color32; 2],
    /// Eyelid color per eye (the fur right around it).
    pub lids: [Color32; 2],
    /// Dark faces get a soft glowing eye rim instead of a dark contour.
    pub dark_face: bool,
    pub whisker: Option<Color32>,
    pub nose: Option<Color32>,
    pub mouth: Option<Color32>,
    /// Main marking color (bib, spots, patches, muzzle...).
    pub marking: Color32,
    /// Secondary marking color.
    pub accent: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

const BLACK: Color32 = rgb(17, 17, 19);
const WHITE: Color32 = rgb(246, 246, 242);
const LIME: Color32 = rgb(206, 240, 74);
const LIME_GLOW: Color32 = Color32::from_rgba_premultiplied(165, 200, 48, 90);
const PUPIL: Color32 = rgb(15, 15, 17);
const PALE_RIM: Color32 = rgb(214, 218, 212);
const PALE_WHISKER: Color32 = rgb(220, 224, 218);

impl PetKind {
    /// Pets offered in the picker and the menu. Capybara, dinosaur and alien still exist (and
    /// still work from an old config or `--pet`), they are just not offered for now.
    pub const VISIBLE: [PetKind; 6] = [
        PetKind::BlackCat,
        PetKind::Tuxedo,
        PetKind::Bengal,
        PetKind::Calico,
        PetKind::Siamese,
        PetKind::Dog,
    ];

    /// Every pet the renderer knows, including the hidden ones.
    #[cfg(test)]
    pub const ALL: [PetKind; 9] = [
        PetKind::BlackCat,
        PetKind::Tuxedo,
        PetKind::Bengal,
        PetKind::Calico,
        PetKind::Siamese,
        PetKind::Dog,
        PetKind::Capybara,
        PetKind::Dino,
        PetKind::Alien,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PetKind::BlackCat => "Gato preto",
            PetKind::Tuxedo => "Frajola",
            PetKind::Bengal => "Bengal",
            PetKind::Calico => "Tricolor",
            PetKind::Dog => "Caramelo",
            PetKind::Capybara => "Capivara",
            PetKind::Siamese => "Siamês",
            PetKind::Dino => "Dinossauro",
            PetKind::Alien => "Alienígena",
        }
    }

    fn index(self) -> usize {
        Self::VISIBLE.iter().position(|k| *k == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::VISIBLE[(self.index() + 1) % Self::VISIBLE.len()]
    }

    pub fn prev(self) -> Self {
        Self::VISIBLE[(self.index() + Self::VISIBLE.len() - 1) % Self::VISIBLE.len()]
    }

    /// Two representative coat colors, for the little swatches in the picker.
    pub fn swatch(self) -> [Color32; 2] {
        let st = self.style();
        match self {
            PetKind::BlackCat => [st.body, st.eye],
            PetKind::Calico => [st.marking, st.accent],
            PetKind::Siamese => [st.body, st.eye],
            PetKind::Dino | PetKind::Alien => [st.body, st.accent],
            _ => [st.body, st.marking],
        }
    }

    pub fn style(self) -> PetStyle {
        match self {
            PetKind::BlackCat => PetStyle {
                species: Species::Cat,
                body: BLACK,
                rim: PALE_RIM,
                outline: None,
                paw: BLACK,
                paw_outline: None,
                hind_paw: BLACK,
                ears: [BLACK; 2],
                tail: BLACK,
                tail_tip: None,
                eye: LIME,
                eye_ring: LIME_GLOW,
                pupil: PUPIL,
                eye_closed: [LIME; 2],
                lids: [BLACK; 2],
                dark_face: true,
                whisker: Some(PALE_WHISKER),
                nose: None,
                mouth: None,
                marking: BLACK,
                accent: BLACK,
            },
            PetKind::Tuxedo => PetStyle {
                species: Species::Cat,
                body: BLACK,
                rim: PALE_RIM,
                outline: None,
                paw: WHITE,
                paw_outline: Some(rgb(70, 70, 76)),
                hind_paw: WHITE,
                ears: [BLACK; 2],
                tail: BLACK,
                tail_tip: Some(WHITE),
                eye: rgb(230, 228, 84),
                eye_ring: Color32::from_rgba_premultiplied(190, 186, 60, 90),
                pupil: PUPIL,
                eye_closed: [rgb(230, 228, 84); 2],
                lids: [BLACK; 2],
                dark_face: true,
                whisker: Some(PALE_WHISKER),
                nose: Some(rgb(238, 150, 162)),
                mouth: Some(rgb(120, 120, 126)),
                marking: WHITE,
                accent: WHITE,
            },
            PetKind::Bengal => {
                let body = rgb(216, 152, 74);
                let dark = rgb(92, 52, 24);
                PetStyle {
                    species: Species::Cat,
                    body,
                    rim: rgb(124, 76, 36),
                    outline: None,
                    paw: body,
                    paw_outline: None,
                    hind_paw: body,
                    ears: [body; 2],
                    tail: body,
                    tail_tip: Some(dark),
                    eye: rgb(178, 216, 98),
                    eye_ring: rgb(70, 40, 20),
                    pupil: PUPIL,
                    eye_closed: [rgb(70, 40, 20); 2],
                    lids: [body; 2],
                    dark_face: false,
                    whisker: Some(rgb(96, 58, 30)),
                    nose: Some(rgb(176, 84, 64)),
                    mouth: Some(rgb(110, 64, 34)),
                    marking: dark,
                    accent: rgb(246, 224, 184),
                }
            }
            PetKind::Calico => {
                let body = rgb(248, 244, 236);
                let orange = rgb(232, 140, 48);
                let black = rgb(38, 34, 36);
                let line = rgb(88, 70, 58);
                PetStyle {
                    species: Species::Cat,
                    body,
                    rim: rgb(176, 156, 138),
                    outline: Some(line),
                    paw: body,
                    paw_outline: Some(line),
                    hind_paw: body,
                    ears: [orange, black],
                    tail: black,
                    tail_tip: Some(orange),
                    eye: rgb(206, 224, 84),
                    eye_ring: rgb(60, 45, 35),
                    pupil: PUPIL,
                    eye_closed: [rgb(70, 45, 25), rgb(244, 226, 160)],
                    lids: [orange, black],
                    dark_face: false,
                    whisker: Some(rgb(126, 110, 100)),
                    nose: Some(rgb(238, 150, 162)),
                    mouth: Some(rgb(150, 122, 112)),
                    marking: orange,
                    accent: black,
                }
            }
            PetKind::Siamese => {
                let body = rgb(240, 226, 200);
                let point = rgb(74, 50, 40);
                let blue = rgb(104, 178, 240);
                PetStyle {
                    species: Species::Cat,
                    body,
                    rim: rgb(172, 142, 116),
                    outline: Some(rgb(112, 86, 68)),
                    paw: point,
                    paw_outline: None,
                    hind_paw: point,
                    ears: [point; 2],
                    tail: point,
                    tail_tip: None,
                    eye: blue,
                    eye_ring: rgb(30, 20, 16),
                    pupil: PUPIL,
                    eye_closed: [rgb(150, 205, 245); 2],
                    lids: [point; 2],
                    dark_face: false,
                    whisker: Some(rgb(176, 156, 140)),
                    nose: Some(rgb(196, 134, 134)),
                    mouth: Some(rgb(158, 128, 112)),
                    marking: point,
                    accent: point,
                }
            }
            PetKind::Dino => {
                let body = rgb(112, 186, 96);
                let dark = rgb(48, 104, 56);
                PetStyle {
                    species: Species::Dino,
                    body,
                    rim: rgb(62, 124, 62),
                    outline: None,
                    paw: body,
                    paw_outline: None,
                    hind_paw: body,
                    ears: [body; 2],
                    tail: body,
                    tail_tip: None,
                    eye: rgb(252, 252, 244),
                    eye_ring: dark,
                    pupil: rgb(24, 34, 26),
                    eye_closed: [dark; 2],
                    lids: [body; 2],
                    dark_face: false,
                    whisker: None,
                    nose: Some(dark),
                    mouth: Some(dark),
                    marking: rgb(216, 234, 152),
                    accent: rgb(242, 142, 62),
                }
            }
            PetKind::Alien => {
                let body = rgb(140, 214, 156);
                let dark = rgb(40, 92, 74);
                PetStyle {
                    species: Species::Alien,
                    body,
                    rim: rgb(66, 140, 100),
                    outline: None,
                    paw: body,
                    paw_outline: None,
                    hind_paw: body,
                    ears: [body; 2],
                    tail: body,
                    tail_tip: None,
                    eye: rgb(18, 22, 30),
                    eye_ring: rgb(18, 22, 30),
                    pupil: rgb(18, 22, 30),
                    eye_closed: [dark; 2],
                    lids: [body; 2],
                    dark_face: false,
                    whisker: None,
                    nose: Some(dark),
                    mouth: Some(dark),
                    marking: rgb(196, 240, 196),
                    accent: rgb(255, 104, 196),
                }
            }
            PetKind::Dog => {
                let body = rgb(220, 162, 94);
                let cream = rgb(247, 230, 198);
                let brown = rgb(146, 96, 50);
                PetStyle {
                    species: Species::Dog,
                    body,
                    rim: brown,
                    outline: None,
                    paw: cream,
                    paw_outline: Some(brown),
                    hind_paw: cream,
                    ears: [rgb(152, 98, 52); 2],
                    tail: body,
                    tail_tip: Some(cream),
                    eye: rgb(44, 28, 20),
                    eye_ring: rgb(44, 28, 20),
                    pupil: rgb(44, 28, 20),
                    eye_closed: [rgb(60, 36, 22); 2],
                    lids: [body; 2],
                    dark_face: false,
                    whisker: None,
                    nose: Some(rgb(30, 24, 24)),
                    mouth: Some(rgb(110, 66, 40)),
                    marking: cream,
                    accent: rgb(236, 120, 130),
                }
            }
            PetKind::Capybara => {
                let body = rgb(174, 122, 72);
                let dark = rgb(110, 74, 44);
                PetStyle {
                    species: Species::Capybara,
                    body,
                    rim: rgb(106, 72, 42),
                    outline: None,
                    paw: dark,
                    paw_outline: None,
                    hind_paw: dark,
                    ears: [body; 2],
                    tail: body,
                    tail_tip: None,
                    eye: rgb(38, 26, 20),
                    eye_ring: rgb(38, 26, 20),
                    pupil: rgb(38, 26, 20),
                    eye_closed: [rgb(52, 34, 22); 2],
                    lids: [body; 2],
                    dark_face: false,
                    whisker: None,
                    nose: Some(rgb(58, 38, 26)),
                    mouth: Some(rgb(84, 54, 32)),
                    marking: rgb(200, 154, 106),
                    accent: rgb(246, 150, 40),
                }
            }
        }
    }
}
