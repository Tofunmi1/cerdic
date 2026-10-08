# Liquid Glass research and browser implementation

The first UI used white gradients, a uniform 18px blur, and borders on almost every panel. It lacked an optical lens and interaction lighting. That made it read as frosted cards.

Apple describes Liquid Glass as a functional layer above content. Lensing bends the background; highlights respond to form and interaction; tint and shadows support legibility. Controls use rounded, concentric shapes. Content keeps standard surfaces, and nested glass is discouraged. The regular material is the appropriate starting point for a workspace with text and variable scene backgrounds. These principles come from [Apple's Materials guidelines](https://developer.apple.com/design/human-interface-guidelines/materials) and [Meet Liquid Glass](https://developer.apple.com/videos/play/wwdc2025/219/).

The [reference project](https://github.com/nikdelvin/liquid-glass) separates foreground content from a backdrop layer and uses SVG displacement. Its [source](https://github.com/nikdelvin/liquid-glass/blob/main/src/utils/liquidGlass.ts) was inspected. Spatial Lab uses its own rounded-distance-field lens implementation, rather than copying its Astro components or utility code.

## Applied changes

- Glass is reserved for navigation, scene tools, object selection and the inspector. Branding and chat content have quieter surfaces. Fields inside glass use thin fills with no extra optical layer.
- `src/ui/glassOptics.ts` creates an edge-only lens map. RG channels encode the rounded boundary normal, with neutral values in the center. A smooth rim profile controls displacement.
- `src/ui/useLiquidGlass.ts` builds a distinct SVG filter for each surface. A separate pseudo-element filters the live backdrop; labels and icons stay sharp. Maps are resized with their surfaces and capped at 256 pixels on their longest side. Pointer movement updates CSS highlights rather than regenerating images or reading the WebGL framebuffer.
- Toolbar selection moves as one rounded fill. Press feedback and menu opening use restrained motion. Form focus is visible.
- Reduced transparency and increased contrast use a solid material; reduced motion disables elastic animation and pointer lighting.

## Limits and verification

This is a browser approximation, not Apple's native renderer. It does not reproduce automatic native contrast switching, device-motion lighting, or every native morphing behavior. SVG backdrop filter support also varies: [MDN's documentation](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/backdrop-filter) describes the API, while the [browser compatibility discussion](https://github.com/mdn/browser-compat-data/issues/24110) documents SVG differences. The lens path is enabled for Chromium; other engines retain a standard blur fallback. A successful CSS syntax check alone does not prove optical rendering support.

Tests cover the map's neutral center, opposite edges and rounded normals; browser checks cover filter attachment, pointer lighting, toolbar state, and accessibility preferences. A striped-background probe compares an actual refracted render with the blur-only baseline. Functional workspace tests still cover chat, edits, history, persistence and gizmo dragging.
