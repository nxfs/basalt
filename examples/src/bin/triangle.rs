extern crate basalt;

use basalt::Basalt;

fn main() {
    Basalt::initialize_iml(
        basalt::Options::default()
			.ignore_dpi(true)
			.window_size(300, 300)
			.title("Basalt"),
        Box::new(move |bst_iml_res| {
            let (basalt, iml) = bst_iml_res.unwrap();

            basalt.wait_for_exit().unwrap();
        })
    );
}