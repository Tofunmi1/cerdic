# Animation operators

- `anim.track@1` authors position, rotation, scale, glow, visibility or colour keyframes.
- `anim.wave@1` creates a looping oscillation.
- `anim.drop@1` bakes a released part's gravity, floor impacts, bounce and rest into one-shot linear position keys; nonzero `spin` adds a rotation track about the geometry's bounding-box centre. Floor contact uses the rest bounding-box bottom and does not account for tumbling. The bake uses fixed 1/480-second semi-implicit steps, at most six impacts, and 40% horizontal-speed and spin loss at each impact.
