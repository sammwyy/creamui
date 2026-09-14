bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct DirtyFlags: u16 {
        const STRUCTURE = 1 << 0;
        const STYLE     = 1 << 1;
        const MEASURE   = 1 << 2;
        const LAYOUT    = 1 << 3;
        const PAINT     = 1 << 4;
        const HIT_TEST  = 1 << 5;
        const COMPOSITE = 1 << 6;
        const ACCESS    = 1 << 7;
    }
}
