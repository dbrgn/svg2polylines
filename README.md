# svg2polylines

Convert an SVG file to a list of polylines (aka polygonal chains or polygonal
paths).

This can be used e.g. for simple drawing robot that just support drawing
straight lines and liftoff / drop pen commands.

## Usage

Signature:

    fn svg2polylines::parse(&str) -> Result<Vec<Polyline>>;

...with the following type definition:

    type Polyline = Vec<(f64, f64)>;
