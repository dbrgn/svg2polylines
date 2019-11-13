pub const HELP: &'static str = "svg2polylines

USAGE:
    svg2polylines [OPTIONS] [INPUT]

OPTIONS:
    -h, --help\t\tPrint this message

Returns a 3D JSON array.";

fn main() {
    fn inner() -> Result<(), Box<dyn std::error::Error>> {
        let mut input = None;

        for arg in std::env::args().skip(1) {
            match arg.as_str() {
                "-h" | "--help" => {
                    println!("{}", HELP);
                    return Ok(());
                }
                _ => {
                    input = Some(arg);
                }
            }
        }

        let mut input = if let Some(input) = input {
            input
        } else {
            let mut buffer = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)?;
            buffer
        };

        if std::path::Path::new(&input).exists() {
            input = std::fs::read_to_string(&input)?;
        }

        let lines = svg2polylines::parse(&input)?;

        let lines_len = lines.len();

        let mut out = String::with_capacity(lines_len * 36);
        
        out.push_str("[\r\n");
        
        for (idx, line) in lines.into_iter().enumerate() {
            out.push_str("  [\r\n");
            let line_len = line.len();
            for (idx, svg2polylines::CoordinatePair { x, y }) in line.into_iter().enumerate() {
                out.push_str(&format!("    [{}, {}]", x, y));
                if idx != (line_len - 1) {
                    out.push_str(",");
                }
                out.push_str("\r\n");
            }
            if idx != (lines_len - 1) {
                out.push_str(",");
            }
            out.push_str("  ]\r\n");
        }
        out.push_str("]");

        println!("{}", out);

        Ok(())
    }

    if let Err(e) = inner() {
        eprintln!("{}", e);
        std::process::exit(2);
    }
}
