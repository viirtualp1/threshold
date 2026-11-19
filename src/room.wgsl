struct Room {
    min: vec4<f32>,
    max: vec4<f32>,
    ranges: vec4<u32>,
}

struct Door {
    bottom: vec4<f32>,
    inward: vec4<f32>,
    axis: vec4<f32>,
    link: vec4<u32>,
}

struct Prop {
    min: vec4<f32>,
    max: vec4<f32>,
}

struct Sphere {
    center: vec4<f32>,
    room: vec4<u32>,
}

struct Scene {
    rooms: array<Room, 3>,
    doors: array<Door, 6>,
    props: array<Prop, 26>,
    glyphs: array<vec2<u32>, 64>,
}

struct Frame {
    eye: vec4<f32>,
    forward: vec4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    display: vec4<f32>,
    flags: vec4<u32>,
    balls: array<Sphere, 7>,
    text: array<array<vec4<u32>, 3>, 10>,
}

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> scene: Scene;
@group(0) @binding(2) var artwork: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;

const WALL: u32 = 0u;
const FURNITURE: u32 = 1u;
const BALL: u32 = 2u;
const HOPS: u32 = 8u;
const EPS: f32 = 0.00001;

const DOOR_COLORS = array<vec3<f32>, 3>(
    vec3(1.0, 0.49, 0.13),
    vec3(0.12, 0.79, 1.0),
    vec3(0.42, 0.95, 0.65),
);

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct Hit {
    distance: f32,
    normal: vec3<f32>,
    kind: u32,
    index: u32,
}

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Vertex {
    let x = f32((index & 1u) << 1u);
    let y = f32(index & 2u);
    return Vertex(vec4(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0), vec2(x, y));
}

fn inverse(ray: vec3<f32>) -> vec3<f32> {
    return 1.0 / select(ray, select(vec3(-1e-8), vec3(1e-8), ray >= vec3(0.0)), abs(ray) < vec3(1e-8));
}

fn near_box(origin: vec3<f32>, reciprocal: vec3<f32>, prop: Prop) -> vec4<f32> {
    let a = (prop.min.xyz - origin) * reciprocal;
    let b = (prop.max.xyz - origin) * reciprocal;
    let lo = min(a, b);
    let hi = max(a, b);
    let near = max(lo.x, max(lo.y, lo.z));
    let far = min(hi.x, min(hi.y, hi.z));

    if near > far || far <= EPS {
        return vec4(0.0, 0.0, 0.0, 1e20);
    }

    var side = lo;
    var distance = near;
    var sign_factor = -1.0;
    if near <= EPS {
        side = hi;
        distance = far;
        sign_factor = 1.0;
    }

    var normal = vec3(0.0, 0.0, sign(reciprocal.z));
    if distance == side.x {
        normal = vec3(sign(reciprocal.x), 0.0, 0.0);
    } else if distance == side.y {
        normal = vec3(0.0, sign(reciprocal.y), 0.0);
    }
    return vec4(normal * sign_factor, distance);
}

fn wall_hit(origin: vec3<f32>, ray: vec3<f32>, reciprocal: vec3<f32>, room: Room) -> Hit {
    let planes = select(room.min.xyz, room.max.xyz, ray >= vec3(0.0));
    let distances = (planes - origin) * reciprocal;

    if distances.x <= min(distances.y, distances.z) {
        return Hit(distances.x, vec3(-sign(ray.x), 0.0, 0.0), WALL, 0u);
    }
    if distances.y <= distances.z {
        return Hit(distances.y, vec3(0.0, -sign(ray.y), 0.0), WALL, 0u);
    }
    return Hit(distances.z, vec3(0.0, 0.0, -sign(ray.z)), WALL, 0u);
}

fn sphere_hit(origin: vec3<f32>, ray: vec3<f32>, sphere: Sphere, index: u32) -> Hit {
    let center = sphere.center.xyz;
    let offset = origin - center;
    let along = dot(offset, ray);
    let across = offset - ray * along;
    let disc = sphere.center.w * sphere.center.w - dot(across, across);

    if disc < 0.0 {
        return Hit(1e20, vec3(0.0), BALL, index);
    }

    let extent = sqrt(disc);
    var distance = -along - extent;
    if distance <= EPS {
        distance = -along + extent;
    }
    let normal = (origin + ray * distance - center) / sphere.center.w;
    return Hit(select(1e20, distance, distance > EPS), normal, BALL, index);
}

fn closest(origin: vec3<f32>, ray: vec3<f32>, room_id: u32) -> Hit {
    let room = scene.rooms[room_id];
    let reciprocal = inverse(ray);
    var hit = wall_hit(origin, ray, reciprocal, room);

    for (var i = room.ranges.z; i < room.ranges.w; i++) {
        let candidate = near_box(origin, reciprocal, scene.props[i]);
        if candidate.w < hit.distance {
            hit = Hit(candidate.w, candidate.xyz, FURNITURE, i);
        }
    }

    for (var i = 0u; i < frame.flags.y; i++) {
        let sphere = frame.balls[i];
        if sphere.room.x == room_id {
            let candidate = sphere_hit(origin, ray, sphere, i);
            if candidate.distance < hit.distance {
                hit = candidate;
            }
        }
    }

    return hit;
}

fn door_coords(point: vec3<f32>, door: Door) -> vec2<f32> {
    let offset = point - door.bottom.xyz;
    return vec2(-dot(offset, door.axis.xyz), offset.y);
}

fn through_door(point: vec3<f32>, normal: vec3<f32>, room_id: u32) -> u32 {
    let room = scene.rooms[room_id];

    for (var i = room.ranges.x; i < room.ranges.y; i++) {
        let door = scene.doors[i];
        if dot(door.inward.xyz, normal) > 0.9 {
            let local = door_coords(point, door);
            if abs(local.x) < door.bottom.w && local.y >= 0.0 && local.y < door.inward.w {
                return i;
            }
        }
    }
    return 6u;
}

fn transform(vector: vec3<f32>, entry: Door, exit: Door) -> vec3<f32> {
    return -exit.axis.xyz * dot(vector, entry.axis.xyz)
        + vec3(0.0, vector.y, 0.0)
        - exit.inward.xyz * dot(vector, entry.inward.xyz);
}

fn glyph(code: u32, point: vec2<i32>) -> f32 {
    if code < 32u || code >= 96u || any(point < vec2(0)) || point.x >= 5 || point.y >= 7 {
        return 0.0;
    }

    let bits = scene.glyphs[code - 32u];
    let row = select(bits.x >> (u32(point.y % 6) * 5u), bits.y, point.y == 6);
    return f32((row >> u32(4 - point.x)) & 1u);
}

fn print_line(line: u32, position: vec2<f32>) -> f32 {
    if any(position < vec2(0.0)) || position.x >= 288.0 || position.y >= 7.0 {
        return 0.0;
    }

    let cell = vec2<i32>(position);
    let column = u32(cell.x / 6);
    let group = frame.text[line][column / 16u];
    let code = (group[(column / 4u) % 4u] >> ((column % 4u) * 8u)) & 255u;
    return glyph(code, vec2(cell.x % 6, cell.y));
}

fn seams(coords: vec2<f32>, width: f32) -> f32 {
    let edge = abs(fract(coords + 0.5) - 0.5);
    return 1.0 - smoothstep(width * 0.6, width * 1.55, min(edge.x, edge.y));
}

fn noise(point: vec2<f32>) -> f32 {
    let p = fract(vec3(point.x, point.y, point.x) * 0.1031);
    let mixed = p + dot(p, p.yzx + 33.33);
    return fract((mixed.x + mixed.y) * mixed.z);
}

fn fixtures(point: vec3<f32>, room_id: u32) -> f32 {
    if room_id == 2u {
        return select(0.0, 1.0, abs(point.x + 1.18) < 0.36 && abs(point.z) < 1.45);
    }

    let center = select(2.4, 2.9, room_id == 0u);
    return select(0.0, 1.0, abs(abs(point.x) - center) < 0.10 && abs(point.z) < 3.5);
}

fn architecture(position: vec3<f32>, normal: vec3<f32>, room_id: u32, distance: f32) -> vec4<f32> {
    let wall_colors = array<vec3<f32>, 3>(vec3(0.59, 0.51, 0.42), vec3(0.46, 0.54, 0.51), vec3(0.34, 0.44, 0.40));
    let coordinate = select(position.z, position.x, abs(normal.z) > 0.5);
    let grain = (noise(floor(vec2(coordinate, position.y) * 230.0)) - 0.5) * 0.06 / (1.0 + distance * 0.32);
    var color = wall_colors[room_id] * (1.0 + grain);
    var emission = 0.0;

    if normal.y > 0.5 {
        let joint = seams(position.xz * select(1.25, 0.80, room_id == 0u), 0.012);
        let tile = noise(floor(position.xz * select(1.25, 0.80, room_id == 0u) + 0.5));
        let grit = noise(floor(position.xz * 180.0)) - 0.5;
        let floor_colors = array<vec3<f32>, 3>(vec3(0.23, 0.18, 0.14), vec3(0.26, 0.31, 0.27), vec3(0.12, 0.15, 0.15));
        color = floor_colors[room_id] * (0.86 + tile * 0.23 - joint * 0.35 + grit * 0.14 / (1.0 + distance));
    } else if normal.y < -0.5 {
        color *= 0.66 - seams(position.xz * 0.7, 0.012) * 0.27;
        if fixtures(position, room_id) > 0.0 {
            return vec4(1.0, 0.88, 0.67, 2.8);
        }
    } else {
        let joint = seams(vec2(coordinate * 0.40 + 0.11, (position.y - 0.12) * 0.24), 0.008);
        color *= 1.0 - joint * 0.17;
        if room_id == 0u && position.y < 0.91 {
            color *= vec3(0.50, 0.57, 0.54);
        }
        if position.y < 0.115 {
            color *= 0.24;
        }
        if room_id == 1u && normal.x < -0.5 && position.z < -1.25 && position.z > -4.2 && position.y > 1.42 && position.y < 3.18 {
            let frame_distance = min(min(position.z + 4.2, -1.25 - position.z), min(position.y - 1.42, 3.18 - position.y));
            if frame_distance < 0.08 || abs(position.z + 2.72) < 0.026 {
                return vec4(0.035, 0.07, 0.07, 0.0);
            }
            let gradient = smoothstep(1.37, 3.27, position.y);
            return vec4(mix(vec3(0.32, 0.60, 0.58), vec3(0.62, 0.83, 0.77), gradient), 0.72);
        }
    }

    let room = scene.rooms[room_id];
    for (var i = room.ranges.x; i < room.ranges.y; i++) {
        let door = scene.doors[i];
        let color_id = door.link.z;
        let ink = DOOR_COLORS[color_id];
        let unit = door.bottom.w;
        let p = door_coords(position, door) / unit;
        let top = door.inward.w / unit;

        if dot(normal, door.inward.xyz) > 0.9 {
            let edge = max(abs(p.x) - 1.0, p.y - top);
            if edge >= -0.00001 && edge < 0.15 {
                if edge > 0.047 && edge < 0.071 {
                    return vec4(ink, 1.45);
                }
                return vec4(vec3(0.040, 0.062, 0.065) + ink * 0.13, 0.0);
            }

            let plaque = vec2(p.x + 0.72, top + 0.47 - p.y);
            if abs(p.x) < 0.86 && plaque.y > -0.062 && plaque.y < 0.224 {
                let dots = print_line(7u + color_id, plaque / 0.021);
                if dots > 0.0 {
                    return vec4(mix(vec3(0.86), ink, 0.74), 0.54);
                }
                return vec4(0.022, 0.032, 0.035, 0.0);
            }

            color += ink * (0.16 * exp(-6.0 * max(edge, 0.0)));
        }

        if normal.y > 0.5 {
            let depth = dot(position - door.bottom.xyz, door.inward.xyz) / unit;
            if depth < 1.80 && abs(p.x) < 0.99 {
                color += ink * (0.15 * exp(-2.1 * max(depth, 0.0)));
                let dash = abs(fract(depth * 5.0) - 0.5) < 0.22;
                if abs(abs(p.x) - 0.87) < 0.016 && dash {
                    color += ink * 0.26;
                    emission = 0.19;
                }
            }
        }
    }

    return vec4(color, emission);
}

fn furniture(position: vec3<f32>, normal: vec3<f32>, prop: Prop) -> vec4<f32> {
    let material = u32(prop.min.w);
    let p = position - prop.min.xyz;
    let coord = select(p.z, p.x, abs(normal.z) > 0.5);
    let detail = noise(floor(vec2(coord, p.y) * 180.0));

    switch material {
        case 4u: { return vec4(vec3(0.15, 0.083, 0.044) * (0.85 + 0.20 * noise(floor(vec2(coord * 23.0, p.y * 360.0)))), 0.0); }
        case 5u: { return vec4(vec3(0.064, 0.081, 0.084) * (0.9 + detail * 0.17), 0.0); }
        case 6u: { return vec4(vec3(0.42, 0.43, 0.38) * (0.90 + detail * 0.14), 0.0); }
        case 7u: { return vec4(vec3(0.51, 0.25, 0.10) * (0.90 + detail * 0.14), 0.0); }
        case 8u: {
            let seam = abs(fract(coord * 1.3) - 0.5);
            let handle = p.y > 0.71 && p.y < 0.735 && seam > 0.22 && seam < 0.39;
            if handle && abs(normal.y) < 0.5 {
                return vec4(0.36, 0.31, 0.22, 0.0);
            }
            return vec4(vec3(0.105, 0.215, 0.171) * select(1.0, 0.67, (seam < 0.012 || p.y < 0.089) && abs(normal.y) < 0.5), 0.0);
        }
        case 9u: { return vec4(vec3(0.24, 0.28, 0.29) * (0.79 + abs(normal.x) * 0.29), 0.0); }
        default: { return vec4(1.0, 0.68, 0.29, 1.7); }
    }
}

fn surface_color(position: vec3<f32>, hit: Hit, room_id: u32) -> vec4<f32> {
    if hit.kind == WALL {
        return architecture(position, hit.normal, room_id, hit.distance);
    }
    if hit.kind == FURNITURE {
        return furniture(position, hit.normal, scene.props[hit.index]);
    }

    let lathe = hit.normal.y + hit.normal.x * 0.43;
    let stripe = 1.0 - smoothstep(0.055, 0.079, abs(lathe - 0.19));
    let thin = 1.0 - smoothstep(0.012, 0.026, abs(lathe - 0.32));
    let rubber = 0.89 + noise(floor(hit.normal.xz * 120.0)) * 0.18;
    return vec4(mix(vec3(0.85, 0.115, 0.035) * rubber, vec3(0.98, 0.73, 0.39), max(stripe, thin)), 0.0);
}

fn shade(position: vec3<f32>, ray: vec3<f32>, hit: Hit, room_id: u32) -> vec3<f32> {
    let base = surface_color(position, hit, room_id);
    let lights = array<vec3<f32>, 3>(vec3(0.0, 3.83, 1.87), vec3(0.1, 3.64, 0.35), vec3(-0.96, 3.34, 0.0));
    let tints = array<vec3<f32>, 3>(vec3(1.0, 0.83, 0.61), vec3(0.78, 0.92, 0.84), vec3(0.76, 1.0, 0.87));
    let n = hit.normal;
    let light = lights[room_id] - position;
    let distance = length(light);
    let heading = light / max(distance, EPS);
    let incident = max(dot(n, heading), 0.0);
    var visibility = 1.0;

    if incident > 0.0 && base.a < 0.3 {
        let block = closest(position + n * 0.00018, heading, room_id);
        if block.distance < distance - 0.05 {
            visibility = 0.28;
        }
    }

    let room = scene.rooms[room_id];
    let walls = min(position.xz - room.min.xz, room.max.xz - position.xz);
    let edge = min(walls.x, walls.y) + min(position.y, room.max.y - position.y);
    let ao = mix(0.74, 1.0, smoothstep(0.0, 0.55, edge));
    let strength = 3.6 / (1.0 + 0.30 * distance * distance);
    let ambient = tints[room_id] * (0.37 + max(n.y, 0.0) * 0.15);
    let specular = pow(max(dot(n, normalize(heading - ray)), 0.0), select(17.0, 42.0, hit.kind == BALL));
    return base.rgb * (ambient * ao + tints[room_id] * incident * strength * visibility + base.a)
        + tints[room_id] * specular * incident * visibility * select(0.085, 0.53, hit.kind == BALL);
}

@fragment
fn room_fragment(input: Vertex) -> @location(0) vec4<f32> {
    let uv = input.uv * 2.0 - 1.0;
    let aspect = frame.display.x / frame.display.y;
    var ray = normalize(frame.forward.xyz + frame.right.xyz * uv.x * aspect * 0.62 - frame.up.xyz * uv.y * 0.62);
    var origin = frame.eye.xyz;
    var room_id = frame.flags.x;
    var color = vec3(0.014, 0.031, 0.036);

    for (var hop = 0u; hop <= HOPS; hop++) {
        let hit = closest(origin, ray, room_id);
        let point = origin + ray * max(0.0, hit.distance);
        var door_id = 6u;
        if hit.kind == WALL && abs(hit.normal.y) < 0.5 {
            door_id = through_door(point, hit.normal, room_id);
        }

        if door_id == 6u {
            let fog = 1.0 - exp(-max(hit.distance, 0.0) * 0.0077);
            color = mix(shade(point, ray, hit, room_id), vec3(0.23, 0.27, 0.24), fog);
            break;
        }
        if hop == HOPS {
            break;
        }

        let entry = scene.doors[door_id];
        let exit = scene.doors[entry.link.x];
        let ratio = exit.bottom.w / entry.bottom.w;
        origin = exit.bottom.xyz + transform(point - entry.bottom.xyz, entry, exit) * ratio + exit.inward.xyz * EPS;
        ray = transform(ray, entry, exit);
        room_id = exit.link.y;
    }

    let grade = 1.0 - 0.14 * dot(uv * vec2(0.69, 0.77), uv * vec2(0.69, 0.77));
    let exposed = color * grade;
    return vec4(exposed / (exposed + vec3(0.66)), 1.0);
}

fn rect(point: vec2<f32>, size: vec2<f32>) -> bool {
    return all(point >= vec2(0.0)) && all(point < size);
}

fn panel(color: vec3<f32>, q: vec2<f32>, dimensions: vec2<f32>, accent: vec3<f32>) -> vec3<f32> {
    if !rect(q, dimensions) {
        return color;
    }
    if q.x < 2.0 {
        return mix(vec3(0.027, 0.036, 0.039), accent, 0.62);
    }
    if q.y < 1.0 || q.y > dimensions.y - 1.0 || q.x > dimensions.x - 1.0 {
        return mix(color, vec3(0.34, 0.38, 0.36), 0.58);
    }
    return mix(color, vec3(0.014, 0.021, 0.023), 0.91);
}

fn label(color: vec3<f32>, line: u32, point: vec2<f32>, scale: f32, ink: vec3<f32>) -> vec3<f32> {
    return mix(color, ink, print_line(line, point / scale));
}

@fragment
fn present_fragment(input: Vertex) -> @location(0) vec4<f32> {
    var color = textureSample(artwork, linear_sampler, input.uv).rgb;
    let unit = max(0.40, min(frame.display.x / 1100.0, frame.display.y / 650.0));
    let screen = frame.display.xy / unit;
    let p = input.uv * screen;
    let warm = vec3(0.91, 0.61, 0.28);
    let white = vec3(0.84, 0.86, 0.81);
    let muted = vec3(0.41, 0.51, 0.48);
    let state = frame.flags.z;
    let room_accent = array<vec3<f32>, 3>(warm, DOOR_COLORS[1], DOOR_COLORS[2])[frame.flags.x];

    let brand = p - vec2(25.0, 23.0);
    color = panel(color, brand, vec2(206.0, 64.0), warm);
    color = label(color, 0u, brand - vec2(14.0, 12.0), 3.0, white);
    color = label(color, 1u, brand - vec2(14.0, 44.0), 1.0, warm);

    let room = p - vec2(screen.x - 192.0, 23.0);
    color = panel(color, room, vec2(167.0, 64.0), room_accent);
    color = label(color, 2u, room - vec2(14.0, 13.0), 2.0, white);
    color = label(color, 3u, room - vec2(14.0, 41.0), 1.0, room_accent);

    let controls = p - vec2(25.0, screen.y - 74.0);
    color = panel(color, controls, vec2(318.0, 51.0), muted);
    color = label(color, 4u, controls - vec2(14.0, 12.0), 1.0, white);
    color = label(color, 5u, controls - vec2(14.0, 31.0), 1.0, muted);

    if state != 0u || frame.flags.w == 0u {
        let action = p - vec2(screen.x - 217.0, screen.y - 56.0);
        color = panel(color, action, vec2(192.0, 33.0), warm);
        color = label(color, 6u, action - vec2(13.0, 13.0), 1.0, white);
    }

    let crosshair = abs(p - screen * 0.5);
    if (max(crosshair.x, crosshair.y) < 5.5 && min(crosshair.x, crosshair.y) < 0.70 && max(crosshair.x, crosshair.y) > 2.0)
        || max(crosshair.x, crosshair.y) < 0.62 {
        color = mix(color, select(white, warm, state != 0u), 0.94);
    }

    if frame.display.w == 0.0 {
        color = pow(max(color, vec3(0.0)), vec3(1.0 / 2.2));
    }
    return vec4(color, 1.0);
}
