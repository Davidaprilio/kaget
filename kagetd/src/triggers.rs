//! Gesture triggers.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Edge {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
    Diagonal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Trigger {
    CornerHold(Corner),
    /// Tap a corner, then touch it again and hold.
    CornerTapHold(Corner),
    CornerSwipe(Corner, Dir),
    SideDrag(Edge, u32),
    Swipe(u32, Dir),
    PinchIn(u32),
    PinchOut(u32),
    Hold(u32),
    Tap(u32),
    DoubleTap(u32),
    TapHold(u32),
}

pub const CORNERS: [Corner; 4] = [Corner::TopLeft, Corner::TopRight, Corner::BottomLeft, Corner::BottomRight];
pub const EDGES: [Edge; 4] = [Edge::Top, Edge::Right, Edge::Bottom, Edge::Left];

impl Corner {
    pub fn name(self) -> &'static str {
        match self {
            Corner::TopLeft => "top_left",
            Corner::TopRight => "top_right",
            Corner::BottomLeft => "bottom_left",
            Corner::BottomRight => "bottom_right",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        CORNERS.into_iter().find(|c| c.name() == s)
    }
    pub fn is_right(self) -> bool {
        matches!(self, Corner::TopRight | Corner::BottomRight)
    }
    pub fn is_bottom(self) -> bool {
        matches!(self, Corner::BottomLeft | Corner::BottomRight)
    }
    /// Swipe directions from this corner: away from its two edges (diagonal: towards the centre),
    /// never off the touchpad.
    pub fn swipe_dirs(self) -> [Dir; 3] {
        let h = if self.is_right() { Dir::Left } else { Dir::Right };
        let v = if self.is_bottom() { Dir::Up } else { Dir::Down };
        [h, v, Dir::Diagonal]
    }
}

impl Edge {
    pub fn name(self) -> &'static str {
        match self {
            Edge::Top => "top",
            Edge::Right => "right",
            Edge::Bottom => "bottom",
            Edge::Left => "left",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        EDGES.into_iter().find(|e| e.name() == s)
    }
}

impl Dir {
    pub fn name(self) -> &'static str {
        match self {
            Dir::Left => "left",
            Dir::Right => "right",
            Dir::Up => "up",
            Dir::Down => "down",
            Dir::Diagonal => "diagonal",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        [Dir::Left, Dir::Right, Dir::Up, Dir::Down, Dir::Diagonal].into_iter().find(|d| d.name() == s)
    }
}

fn fingers(s: &str) -> Result<u32, String> {
    let n: u32 = s.parse().map_err(|_| format!("'{s}' is not a finger count"))?;
    if !(3..=10).contains(&n) {
        return Err("finger count must be between 3 and 10 (2-finger gestures belong to the system)".into());
    }
    Ok(n)
}

impl Trigger {
    pub fn parse(s: &str) -> Result<Self, String> {
        let p: Vec<&str> = s.split('.').collect();
        let bad = || format!("unknown trigger '{s}'");
        Ok(match p.as_slice() {
            ["corner", c, "hold"] => Trigger::CornerHold(Corner::parse(c).ok_or_else(bad)?),
            ["corner", c, "taphold"] => Trigger::CornerTapHold(Corner::parse(c).ok_or_else(bad)?),
            ["corner", c, "swipe", d] => {
                let (c, d) = (Corner::parse(c).ok_or_else(bad)?, Dir::parse(d).ok_or_else(bad)?);
                if !c.swipe_dirs().contains(&d) {
                    return Err(format!("{s}: a corner swipe goes towards the centre, not off the touchpad"));
                }
                Trigger::CornerSwipe(c, d)
            }
            ["sidedrag", e, n] => {
                let n = fingers(n)?;
                if n > 4 {
                    return Err("side drag supports 3 or 4 fingers".into());
                }
                Trigger::SideDrag(Edge::parse(e).ok_or_else(bad)?, n)
            }
            ["swipe", n, d] => {
                let d = Dir::parse(d).ok_or_else(bad)?;
                if d == Dir::Diagonal {
                    return Err(bad());
                }
                Trigger::Swipe(fingers(n)?, d)
            }
            ["pinch", n, "in"] => Trigger::PinchIn(fingers(n)?),
            ["pinch", n, "out"] => Trigger::PinchOut(fingers(n)?),
            ["hold", n] => Trigger::Hold(fingers(n)?),
            ["tap", n] => Trigger::Tap(fingers(n)?),
            ["doubletap", n] => Trigger::DoubleTap(fingers(n)?),
            ["taphold", n] => Trigger::TapHold(fingers(n)?),
            _ => return Err(bad()),
        })
    }

    pub fn fingers(self) -> u32 {
        match self {
            Trigger::CornerHold(_) | Trigger::CornerTapHold(_) | Trigger::CornerSwipe(..) => 1,
            Trigger::SideDrag(_, n) | Trigger::Swipe(n, _) => n,
            Trigger::PinchIn(n) | Trigger::PinchOut(n) => n,
            Trigger::Hold(n) | Trigger::Tap(n) | Trigger::DoubleTap(n) | Trigger::TapHold(n) => n,
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            Trigger::CornerHold(_) | Trigger::CornerTapHold(_) | Trigger::CornerSwipe(..) => "Corners",
            Trigger::SideDrag(..) => "Side drag",
            Trigger::Swipe(..) => "Swipe",
            Trigger::PinchIn(_) | Trigger::PinchOut(_) => "Pinch",
            Trigger::Hold(_) | Trigger::Tap(_) | Trigger::DoubleTap(_) | Trigger::TapHold(_) => "Taps",
        }
    }

    /// Gestures KWin/libinput already use (v1 list).
    pub fn system_owned(self) -> bool {
        match self {
            Trigger::Swipe(n, _) | Trigger::PinchIn(n) | Trigger::PinchOut(n) => n == 3 || n == 4,
            Trigger::Tap(n) => n == 3,
            _ => false,
        }
    }

    pub fn description(self) -> String {
        let f = |n: u32| format!("{n}-finger");
        match self {
            Trigger::CornerHold(c) => format!("Hold a finger in the {} corner", c.name().replace('_', " ")),
            Trigger::CornerTapHold(c) => {
                format!("Tap the {} corner, then touch it again and hold", c.name().replace('_', " "))
            }
            Trigger::CornerSwipe(c, d) => {
                format!("Swipe {} starting in the {} corner", d.name(), c.name().replace('_', " "))
            }
            Trigger::SideDrag(e, n) => format!("{} drag in from the {} edge", f(n), e.name()),
            Trigger::Swipe(n, d) => format!("{} swipe {}", f(n), d.name()),
            Trigger::PinchIn(n) => format!("{} pinch in", f(n)),
            Trigger::PinchOut(n) => format!("{} pinch out", f(n)),
            Trigger::Hold(n) => format!("{} hold", f(n)),
            Trigger::Tap(n) => format!("{} tap", f(n)),
            Trigger::DoubleTap(n) => format!("{} double tap", f(n)),
            Trigger::TapHold(n) => format!("{} tap, then touch and hold", f(n)),
        }
    }

    /// Whether this device can perform the trigger; Err carries the reason.
    pub fn available(self, max_fingers: u32, semi_mt: bool) -> Result<(), String> {
        let n = self.fingers();
        if n == 1 {
            return Ok(());
        }
        if semi_mt {
            return Err("This touchpad cannot track individual fingers (semi-multitouch)".into());
        }
        if n > max_fingers {
            return Err(format!("This touchpad tracks at most {max_fingers} fingers"));
        }
        Ok(())
    }

    /// Every trigger this device supports, for ListTriggers().
    pub fn all(max_fingers: u32) -> Vec<Trigger> {
        let mut v = Vec::new();
        for c in CORNERS {
            v.push(Trigger::CornerHold(c));
            v.push(Trigger::CornerTapHold(c));
            for d in c.swipe_dirs() {
                v.push(Trigger::CornerSwipe(c, d));
            }
        }
        let top = max_fingers.clamp(3, 5);
        for n in 3..=4.min(top) {
            for e in EDGES {
                v.push(Trigger::SideDrag(e, n));
            }
        }
        for n in 3..=top {
            for d in [Dir::Left, Dir::Right, Dir::Up, Dir::Down] {
                v.push(Trigger::Swipe(n, d));
            }
            v.extend([Trigger::PinchIn(n), Trigger::PinchOut(n)]);
            v.extend([Trigger::Hold(n), Trigger::Tap(n), Trigger::DoubleTap(n), Trigger::TapHold(n)]);
        }
        v
    }
}

impl fmt::Display for Trigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Trigger::CornerHold(c) => write!(f, "corner.{}.hold", c.name()),
            Trigger::CornerTapHold(c) => write!(f, "corner.{}.taphold", c.name()),
            Trigger::CornerSwipe(c, d) => write!(f, "corner.{}.swipe.{}", c.name(), d.name()),
            Trigger::SideDrag(e, n) => write!(f, "sidedrag.{}.{n}", e.name()),
            Trigger::Swipe(n, d) => write!(f, "swipe.{n}.{}", d.name()),
            Trigger::PinchIn(n) => write!(f, "pinch.{n}.in"),
            Trigger::PinchOut(n) => write!(f, "pinch.{n}.out"),
            Trigger::Hold(n) => write!(f, "hold.{n}"),
            Trigger::Tap(n) => write!(f, "tap.{n}"),
            Trigger::DoubleTap(n) => write!(f, "doubletap.{n}"),
            Trigger::TapHold(n) => write!(f, "taphold.{n}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all() {
        for t in Trigger::all(5) {
            assert_eq!(Trigger::parse(&t.to_string()).unwrap(), t);
        }
    }

    #[test]
    fn rejects_two_fingers_and_garbage() {
        assert!(Trigger::parse("swipe.2.left").is_err());
        assert!(Trigger::parse("sidedrag.top.5").is_err());
        assert!(Trigger::parse("swipe.3.diagonal").is_err());
        assert!(Trigger::parse("corner.center.hold").is_err());
        assert!(Trigger::parse("corner.top_left.swipe.left").is_err(), "off the touchpad");
        assert!(Trigger::parse("corner.bottom_right.swipe.down").is_err());
        assert!(Trigger::parse("corner.bottom_right.swipe.up").is_ok());
    }

    #[test]
    fn system_owned_set() {
        assert!(Trigger::parse("swipe.4.left").unwrap().system_owned());
        assert!(Trigger::parse("tap.3").unwrap().system_owned());
        assert!(!Trigger::parse("doubletap.3").unwrap().system_owned());
        assert!(!Trigger::parse("swipe.5.up").unwrap().system_owned());
        assert!(!Trigger::parse("sidedrag.top.3").unwrap().system_owned());
    }
}
