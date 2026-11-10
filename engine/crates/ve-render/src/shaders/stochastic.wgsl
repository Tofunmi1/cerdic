// Triangular-grid maths adapted from RedGPU's stochasticSampling.wgsl:
// MIT License
//
// Copyright (c) 2019~ Redcamel
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// to be furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
// Linear barycentric weights, after Heitz and Neyret.
// The integer hash and float conversion are shared with ve_render::stochastic.
fn stochastic_hash(value: u32) -> u32 {
    var h = value ^ (value >> 16u);
    h *= 0x7feb352du;
    h ^= h >> 15u;
    h *= 0x846ca68bu;
    return h ^ (h >> 16u);
}

fn vertex_offset(cell: vec2<i32>, seed: u32) -> vec2<f32> {
    let key = stochastic_hash(bitcast<u32>(cell.x) ^ stochastic_hash(bitcast<u32>(cell.y) ^ seed) ^ 0x9e3779b9u);
    return vec2<f32>(f32(key >> 8u), f32(stochastic_hash(key ^ 0x68bc21ebu) >> 8u)) * (1.0 / 16777216.0);
}

struct StochasticSample {
    uv: vec2<f32>,
    weight: f32,
};

fn stochastic_finite(v: f32) -> f32 {
    // Comparisons also reject NaN; no isFinite builtin is required by WebGL2.
    return select(0.0, v, abs(v) <= 3.402823466e38);
}

fn stochastic_cells(input_uv: vec2<f32>, input_scale: f32, seed: u32) -> array<StochasticSample, 3> {
    let uv = vec2<f32>(stochastic_finite(input_uv.x), stochastic_finite(input_uv.y));
    let scale = clamp(select(1.0, input_scale, abs(input_scale) <= 3.402823466e38), 0.01, 1000.0);
    let p = clamp(uv, vec2<f32>(-1000000.0), vec2<f32>(1000000.0)) * scale;
    let skew = vec2<f32>(p.x - p.y * 0.57735026, p.y * 1.1547005);
    let base = floor(skew);
    let cell = vec2<i32>(base);
    let f = skew - base;
    let w = 1.0 - f.x - f.y;
    var v0 = cell;
    var weights = vec3<f32>(w, f.x, f.y);
    if (w < 0.0) {
        v0 += vec2<i32>(1, 1);
        weights = vec3<f32>(-w, 1.0 - f.y, 1.0 - f.x);
    }
    return array<StochasticSample, 3>(
        StochasticSample(uv + vertex_offset(v0, seed), weights.x),
        StochasticSample(uv + vertex_offset(cell + vec2<i32>(1, 0), seed), weights.y),
        StochasticSample(uv + vertex_offset(cell + vec2<i32>(0, 1), seed), weights.z),
    );
}
