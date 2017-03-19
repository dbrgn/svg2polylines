from cffi import FFI

ffi = FFI()
lib = ffi.dlopen('target/debug/libsvg2polylines.so')

ffi.cdef('''
typedef struct CoordinatePair {
    double x;
    double y;
} CoordinatePair;

typedef struct Polyline {
    CoordinatePair* ptr;
    size_t len;
} Polyline;

uint8_t svg_str_to_polylines(char* svg, Polyline** polylines, size_t* polylines_len);
void free_polylines(Polyline* polylines, size_t polylines_len);
''')

svg_input = b'''
<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg xmlns="http://www.w3.org/2000/svg" version="1.1">
  <g
     transform="translate(-24.666516,-30.77247)"
     id="layer1">
    <path
       id="path4485"
       d="m 70.303571,34.306548 -40.443453,44.601188 65.767856,4.91369 z"
       style="fill:none;stroke:#000000;stroke-width:0.26458332px;stroke-opacity:1" />
    <path
       id="path4487"
       d="m 113.01488,35.818452 h 40.44345 l -39.6875,49.514881 h 40.06548"
       style="fill:none;stroke:#000000;stroke-width:0.26458332px;stroke-opacity:1" />
  </g>
</svg>
'''


def print_polyline(p):
    print('  Length: %d' % p.len)
    print('  Points to: %r' % p.ptr)
    print('  Data:')
    for i in range(p.len):
        print('    (%f, %f)' % (p.ptr[i].x, p.ptr[i].y))


polylines = ffi.new('Polyline**')
polylines_len = ffi.new('size_t*')
lib.svg_str_to_polylines(svg_input, polylines, polylines_len)


print('Found %d polylines!' % polylines_len[0])
for i in range(polylines_len[0]):
    print('Polyline %d:' % (i + 1))
    print_polyline(polylines[0][i])
