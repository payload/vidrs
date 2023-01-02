use miniquad::*;

struct Stage {}

impl EventHandler for Stage {
    fn update(&mut self, _ctx: &mut Context) {}

    fn draw(&mut self, _ctx: &mut Context) {}

    fn char_event(&mut self, _ctx: &mut Context, _character: char, _: KeyMods, _: bool) {}
}

pub fn run_gui() {
    miniquad::start(conf::Conf::default(), |_ctx| Box::new(Stage {}));
}
