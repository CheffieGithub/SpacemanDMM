//! Port of icon smoothing subsystem.

use dm::constants::Constant;
use dmi::Dir;
use minimap::{Sprite, Context, Atom, GetVar, Neighborhood};

// (1 << N) where N is the usual value
const N_NORTH: i32 = 2;
const N_SOUTH: i32 = 4;
const N_EAST: i32 = 16;
const N_WEST: i32 = 256;
const N_NORTHEAST: i32 = 32;
const N_NORTHWEST: i32 = 512;
const N_SOUTHEAST: i32 = 64;
const N_SOUTHWEST: i32 = 1024;

const SMOOTH_TRUE: i32 = 1;  // smooth with exact specified types or just itself
const SMOOTH_MORE: i32 = 2;  // smooth with all subtypes thereof
const SMOOTH_DIAGONAL: i32 = 4;  // smooth diagonally
const SMOOTH_BORDER: i32 = 8;  // smooth with the borders of the map

pub struct IconSmoothing {
    pub mask: i32,
}

impl Default for IconSmoothing {
    fn default() -> Self {
        IconSmoothing { mask: !0 }
    }
}

impl IconSmoothing {
    pub fn handle_smooth<'a>(&self,
        output: &mut Vec<Sprite<'a>>,
        ctx: Context<'a>,
        neighborhood: &Neighborhood<'a, '_>,
        atom: Atom<'a>,
    ) {
        let smooth_flags = self.mask & atom.get_var("smooth", ctx.objtree).to_int().unwrap_or(0);
        if smooth_flags & (SMOOTH_TRUE | SMOOTH_MORE) != 0 {
            let adjacencies = calculate_adjacencies(ctx, neighborhood, &atom, smooth_flags);
            if smooth_flags & SMOOTH_DIAGONAL != 0 {
                diagonal_smooth(output, ctx, neighborhood, &atom, adjacencies);
            } else {
                cardinal_smooth(output, ctx, &atom, adjacencies);
            }
        } else {
            output.push(atom.sprite);
        }
    }
}

fn calculate_adjacencies(ctx: Context, neighborhood: &Neighborhood, atom: &Atom, smooth_flags: i32) -> i32 {
    // TODO: anchored check

    let mut adjacencies = 0;
    let check_one = |direction, flag| {
        if find_type_in_direction(ctx, neighborhood, atom, direction, smooth_flags) {
            flag
        } else {
            0
        }
    };

    for &dir in &[Dir::South, Dir::North, Dir::East, Dir::West] {
        adjacencies |= check_one(dir, 1 << dir.to_int());
    }

    if adjacencies & N_NORTH != 0 {
        if adjacencies & N_WEST != 0 {
            adjacencies |= check_one(Dir::Northwest, N_NORTHWEST);
        }
        if adjacencies & N_EAST != 0 {
            adjacencies |= check_one(Dir::Northeast, N_NORTHEAST);
        }
    }
    if adjacencies & N_SOUTH != 0 {
        if adjacencies & N_WEST != 0 {
            adjacencies |= check_one(Dir::Southwest, N_SOUTHWEST);
        }
        if adjacencies & N_EAST != 0 {
            adjacencies |= check_one(Dir::Southeast, N_SOUTHEAST);
        }
    }

    adjacencies
}

fn find_type_in_direction<'a>(ctx: Context, adjacency: &Neighborhood, source: &Atom, direction: Dir, smooth_flags: i32) -> bool {
    use std::ptr::eq;

    let atom_list = adjacency.offset(direction);
    if atom_list.is_empty() {
        return smooth_flags & SMOOTH_BORDER != 0;
    }

    match source.get_var("canSmoothWith", ctx.objtree) {
        &Constant::List(ref elements) => if smooth_flags & SMOOTH_MORE != 0 {
            // smooth with canSmoothWith + subtypes
            for atom in atom_list {
                let mut path = &atom.type_.path[..];
                while !path.is_empty() {
                    if smoothlist_contains(elements, path) {
                        return true;
                    }
                    path = &path[..path.rfind("/").unwrap()];
                }
            }
        } else {
            // smooth only with exact types in canSmoothWith
            for atom in atom_list {
                if smoothlist_contains(elements, &atom.type_.path) {
                    return true;
                }
            }
        },
        _ => {
            // smooth only with the same type
            for atom in atom_list {
                if eq(atom.type_, source.type_) {
                    return true;
                }
            }
        },
    }
    false
}

fn smoothlist_contains(list: &[(Constant, Option<Constant>)], desired: &str) -> bool {
    for &(ref key, _) in list {
        // TODO: be more specific than to_string
        if key.to_string() == desired {
            return true;
        }
    }
    false
}

fn cardinal_smooth<'a>(output: &mut Vec<Sprite<'a>>, ctx: Context<'a>, source: &Atom<'a>, adjacencies: i32) {
    for &(what, f1, n1, f2, n2, f3) in &[
        ("1", N_NORTH, "n", N_WEST, "w", N_NORTHWEST),
        ("2", N_NORTH, "n", N_EAST, "e", N_NORTHEAST),
        ("3", N_SOUTH, "s", N_WEST, "w", N_SOUTHWEST),
        ("4", N_SOUTH, "s", N_EAST, "e", N_SOUTHEAST),
    ] {
        let name = if (adjacencies & f1 != 0) && (adjacencies & f2 != 0) {
            if (adjacencies & f3) != 0 {
                format!("{}-f", what)
            } else {
                format!("{}-{}{}", what, n1, n2)
            }
        } else if adjacencies & f1 != 0 {
            format!("{}-{}", what, n1)
        } else if adjacencies & f2 != 0 {
            format!("{}-{}", what, n2)
        } else {
            format!("{}-i", what)
        };

        let mut sprite = Sprite {
            icon_state: ctx.bump.alloc(name),
            .. source.sprite
        };
        if let Some(icon) = source.get_var("smooth_icon", ctx.objtree).as_path_str() {
            sprite.icon = icon;
        }
        output.push(sprite);
    }
}

fn diagonal_smooth<'a>(output: &mut Vec<Sprite<'a>>, ctx: Context<'a>, neighborhood: &Neighborhood<'a, '_>, source: &Atom<'a>, adjacencies: i32) {
    let presets = if adjacencies == N_NORTH | N_WEST {
        ["d-se", "d-se-0"]
    } else if adjacencies == N_NORTH | N_EAST {
        ["d-sw", "d-sw-0"]
    } else if adjacencies == N_SOUTH | N_WEST {
        ["d-ne", "d-ne-0"]
    } else if adjacencies == N_SOUTH | N_EAST {
        ["d-nw", "d-nw-0"]
    } else if adjacencies == N_NORTH | N_WEST | N_NORTHWEST {
        ["d-se", "d-se-1"]
    } else if adjacencies == N_NORTH | N_EAST | N_NORTHEAST {
        ["d-sw", "d-sw-1"]
    } else if adjacencies == N_SOUTH | N_WEST | N_SOUTHWEST {
        ["d-ne", "d-ne-1"]
    } else if adjacencies == N_SOUTH | N_EAST | N_SOUTHEAST {
        ["d-nw", "d-nw-1"]
    } else {
        return cardinal_smooth(output, ctx, source, adjacencies);
    };

    // turf underneath
    if dm::objtree::subpath(&source.type_.path, "/turf/closed/wall/") {
        // BYOND memes
        if source
            .get_var("fixed_underlay", ctx.objtree)
            .index(&Constant::string("space"))
            .is_some()
        {
            output.push(Sprite::from_vars(ctx.objtree, &ctx.objtree.expect("/turf/open/space/basic")));
        } else {
            let dir = reverse_ndir(adjacencies).flip();
            let mut needs_plating = true;
            // check direct, then 45deg left, then 45deg right
            'dirs: for &each in &[dir, dir.counterclockwise_45(), dir.clockwise_45()] {
                let atom_list = neighborhood.offset(each);
                for atom in atom_list {
                    if dm::objtree::subpath(&atom.type_.path, "/turf/open/") {
                        output.push(Sprite::from_vars(ctx.objtree, atom));
                        needs_plating = false;
                        break 'dirs;
                    }
                }
            }
            if needs_plating {
                output.push(Sprite::from_vars(ctx.objtree, &ctx.objtree.expect("/turf/open/floor/plating")));
            }
        }
    }

    // the diagonal overlay
    for &each in presets.iter() {
        let mut copy = Sprite {
            icon_state: each,
            .. source.sprite
        };
        if let Some(icon) = source.get_var("smooth_icon", ctx.objtree).as_path_str() {
            copy.icon = icon;
        }
        output.push(copy);
    }
}

fn reverse_ndir(ndir: i32) -> Dir {
    const NW1: i32 = N_NORTH | N_WEST;
    const NW2: i32 = NW1 | N_NORTHWEST;
    const NE1: i32 = N_NORTH | N_EAST;
    const NE2: i32 = NE1 | N_NORTHEAST;
    const SW1: i32 = N_SOUTH | N_WEST;
    const SW2: i32 = SW1 | N_SOUTHWEST;
    const SE1: i32 = N_SOUTH | N_EAST;
    const SE2: i32 = SE1 | N_SOUTHEAST;

    match ndir {
        N_NORTH => Dir::North,
        N_SOUTH => Dir::South,
        N_WEST => Dir::West,
        N_EAST => Dir::East,
        N_SOUTHEAST | SE1 | SE2 => Dir::Southeast,
        N_SOUTHWEST | SW1 | SW2 => Dir::Southwest,
        N_NORTHEAST | NE1 | NE2 => Dir::Northeast,
        N_NORTHWEST | NW1 | NW2 => Dir::Northwest,
        _ => panic!(),
    }
}
