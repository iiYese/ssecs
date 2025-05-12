use crate::component::COMPONENT_INIT_FNS;

pub struct World {
    // ..
}

impl World {
    pub fn new() -> Self {
        let world = Self {};
        for func in COMPONENT_INIT_FNS {
            func(&world);
        }
        world
    }
}
