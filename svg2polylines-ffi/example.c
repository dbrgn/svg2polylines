/**
 * C example.
 *
 * Compile like this:
 *
 *   $ clang example.c -o example -L target/debug/ -lsvg2polylines -Wall -Wextra -g
 *
 * Run like this:
 *
 *   $ LD_LIBRARY_PATH=target/debug/ ./example
 */
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct CoordinatePair {
    double x;
    double y;
} CoordinatePair;

typedef struct Polyline {
    CoordinatePair* ptr;
    size_t len;
} Polyline;

uint8_t svg_str_to_polylines(char* svg,
                             Polyline** out_vec,
                             size_t* out_vec_len);

void print_polyline(Polyline* p) {
    printf("  Address: %p\n", p);
    printf("  Length: %zu\n", p->len);
    printf("  Points to: %p\n", p->ptr);
    printf("  Data:\n");
    for (size_t i = 0; i < p->len; i++) {
        printf("    (%f, %f)\n", p->ptr[i].x, p->ptr[i].y);
    }
}

int main() {
    // SVG data
    char* input = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?><svg xmlns=\"http://www.w3.org/2000/svg\" id=\"svg8\" version=\"1.1\" viewBox=\"0 0 140.1311 56.978192\" height=\"56.978191mm\" width=\"140.1311mm\"><g transform=\"translate(-24.666516,-30.77247)\" id=\"layer1\"><path id=\"path4485\" d=\"m 70.303571,34.306548 -40.443453,44.601188 65.767856,4.91369 z\" style=\"fill:none;fill-rule:evenodd;stroke:#000000;stroke-width:0.26458332px;stroke-linecap:butt;stroke-linejoin:miter;stroke-opacity:1\" /><path id=\"path4487\" d=\"m 113.01488,35.818452 h 40.44345 l -39.6875,49.514881 h 40.06548\" style=\"fill:none;fill-rule:evenodd;stroke:#000000;stroke-width:0.26458332px;stroke-linecap:butt;stroke-linejoin:miter;stroke-opacity:1\" /></g></svg>";

    // Initialize out params
    Polyline* out_vec = NULL;
    size_t out_vec_len = 0;

    // Process data
    uint8_t err = svg_str_to_polylines(input, &out_vec, &out_vec_len);

    // Error handling
    if (err > 0) {
        printf("Error: Return code %d", err);
        exit(err);
    }

    // Print result
    printf("Found %zu polylines!\n", out_vec_len);
    printf("Out vec address: %p\n", out_vec);
    for (size_t i = 0; i < out_vec_len; i++) {
        printf("Polyline %zu:\n", i + 1);
        print_polyline(&out_vec[i]);
    }
}
