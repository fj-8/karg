//! A single-command CLI: `cargo run --example crop -- --help`.

use karg::Cmd;

fn main() {
    let cmd = Cmd::new("crop", env!("CARGO_PKG_VERSION"))
        .about("Crop an image to a rectangle and write it out")
        .flag("verbose", 'v', "Print each step as it happens")
        .long_flag("lossless", "Never re-encode lossy pixels")
        .opt("format", 'f', "Output container")
        .value_name("FMT")
        .default("png")
        .opt("quality", 'q', "JPEG/WEBP quality, 1-100")
        .value_name("N")
        .multi("tag", 't', "Metadata tag as KEY=VALUE, repeatable")
        .value_name("KEY=VALUE")
        .long_opt("profile", "ICC colour profile to embed")
        .hidden()
        .arg("source", "Image to read")
        .arg("target", "Image to write")
        .opt_arg("rect", "W H, or a single crop preset")
        .after_help(
            "Examples:\n  \
             crop a.png out.jpg -f jpeg -q 82 topleft\n  \
             crop -vv --format=webp a.png out.webp center",
        );

    let m = cmd.parse_or_exit();

    if m.flag("verbose") {
        println!(
            "reading {} -> writing {} as {}",
            m.positional(0).unwrap_or("-"),
            m.positional(1).unwrap_or("-"),
            m.get_or("format", "png")
        );
        for tag in m.strings("tag") {
            println!("  tag {tag}");
        }
        println!("  lossless: {}", m.flag("lossless"));
    }

    let quality = m.number_or::<u8>("quality", 90).unwrap();
    println!("quality={quality}");
    println!("preset={:?}", m.positional(2));
}
