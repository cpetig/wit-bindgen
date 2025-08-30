// helper functions for symmetric ABI

use std::collections::HashMap;

use rustc_stable_hash::ExtendedHasher;
use wit_component::DecodedWasm;
use wit_parser::{
    Interface, InterfaceId, Package, PackageName, Resolve, Type, TypeDef, TypeDefKind, TypeOwner,
    World, WorldItem, WorldKey,
};

// figure out whether deallocation is needed in the caller
fn needs_dealloc2(resolve: &Resolve, tp: &Type) -> bool {
    match tp {
        Type::Bool
        | Type::U8
        | Type::U16
        | Type::U32
        | Type::U64
        | Type::S8
        | Type::S16
        | Type::S32
        | Type::S64
        | Type::F32
        | Type::F64
        | Type::Char => false,
        Type::String => true,
        Type::Id(id) => match &resolve.types[*id].kind {
            TypeDefKind::Enum(_) => false,
            TypeDefKind::Record(r) => r.fields.iter().any(|f| needs_dealloc2(resolve, &f.ty)),
            TypeDefKind::Resource => false,
            TypeDefKind::Handle(_) => false,
            TypeDefKind::Flags(_) => false,
            TypeDefKind::Tuple(t) => t.types.iter().any(|f| needs_dealloc2(resolve, f)),
            TypeDefKind::Variant(v) => v
                .cases
                .iter()
                .any(|c| c.ty.map_or(false, |t| needs_dealloc2(resolve, &t))),
            TypeDefKind::Option(tp) => needs_dealloc2(resolve, tp),
            TypeDefKind::Result(r) => {
                r.ok.as_ref()
                    .map_or(false, |tp| needs_dealloc2(resolve, tp))
                    || r.err
                        .as_ref()
                        .map_or(false, |tp| needs_dealloc2(resolve, tp))
            }
            TypeDefKind::List(_l) => true,
            TypeDefKind::Future(_) => todo!(),
            TypeDefKind::Stream(_) => todo!(),
            TypeDefKind::Type(tp) => needs_dealloc2(resolve, tp),
            TypeDefKind::Unknown => false,
            TypeDefKind::FixedSizeList(_, _) => todo!(),
        },
        Type::ErrorContext => todo!(),
    }
}

pub fn needs_dealloc(resolve: &Resolve, args: &[(String, Type)]) -> bool {
    for (_n, t) in args {
        if needs_dealloc2(resolve, t) {
            return true;
        }
    }
    return false;
}

fn has_non_canonical_list2(resolve: &Resolve, ty: &Type, maybe: bool) -> bool {
    match ty {
        Type::Bool
        | Type::U8
        | Type::U16
        | Type::U32
        | Type::U64
        | Type::S8
        | Type::S16
        | Type::S32
        | Type::S64
        | Type::F32
        | Type::F64
        | Type::Char
        | Type::String => false,
        Type::Id(id) => match &resolve.types[*id].kind {
            TypeDefKind::Record(r) => r
                .fields
                .iter()
                .any(|field| has_non_canonical_list2(resolve, &field.ty, maybe)),
            TypeDefKind::Resource | TypeDefKind::Handle(_) | TypeDefKind::Flags(_) => false,
            TypeDefKind::Tuple(t) => t
                .types
                .iter()
                .any(|ty| has_non_canonical_list2(resolve, ty, maybe)),
            TypeDefKind::Variant(var) => var.cases.iter().any(|case| {
                case.ty
                    .as_ref()
                    .map_or(false, |ty| has_non_canonical_list2(resolve, ty, maybe))
            }),
            TypeDefKind::Enum(_) => false,
            TypeDefKind::Option(ty) => has_non_canonical_list2(resolve, ty, maybe),
            TypeDefKind::Result(res) => {
                res.ok
                    .as_ref()
                    .map_or(false, |ty| has_non_canonical_list2(resolve, ty, maybe))
                    || res
                        .err
                        .as_ref()
                        .map_or(false, |ty| has_non_canonical_list2(resolve, ty, maybe))
            }
            TypeDefKind::List(ty) => {
                if maybe {
                    true
                } else {
                    has_non_canonical_list2(resolve, ty, true)
                }
            }
            TypeDefKind::Future(_) | TypeDefKind::Stream(_) => false,
            TypeDefKind::Type(ty) => has_non_canonical_list2(resolve, ty, maybe),
            TypeDefKind::Unknown => false,
            TypeDefKind::FixedSizeList(_, _) => todo!(),
        },
        Type::ErrorContext => todo!(),
    }
}

// fn has_non_canonical_list(resolve: &Resolve, results: &Results) -> bool {
//     match results {
//         Results::Named(vec) => vec
//             .iter()
//             .any(|(_, ty)| has_non_canonical_list2(resolve, ty, false)),
//         Results::Anon(one) => has_non_canonical_list2(resolve, &one, false),
//     }
// }

pub fn has_non_canonical_list(resolve: &Resolve, args: &[(String, Type)]) -> bool {
    args.iter()
        .any(|(_, ty)| has_non_canonical_list2(resolve, ty, false))
}

fn has_non_canonical_list_rust2(resolve: &Resolve, ty: &Type) -> bool {
    match ty {
        Type::Bool
        | Type::U8
        | Type::U16
        | Type::U32
        | Type::U64
        | Type::S8
        | Type::S16
        | Type::S32
        | Type::S64
        | Type::F32
        | Type::F64
        | Type::Char
        | Type::String => false,
        Type::Id(id) => match &resolve.types[*id].kind {
            TypeDefKind::Record(r) => r
                .fields
                .iter()
                .any(|field| has_non_canonical_list_rust2(resolve, &field.ty)),
            TypeDefKind::Resource | TypeDefKind::Handle(_) | TypeDefKind::Flags(_) => false,
            TypeDefKind::Tuple(t) => t
                .types
                .iter()
                .any(|ty| has_non_canonical_list_rust2(resolve, ty)),
            TypeDefKind::Variant(var) => var.cases.iter().any(|case| {
                case.ty
                    .as_ref()
                    .map_or(false, |ty| has_non_canonical_list_rust2(resolve, ty))
            }),
            TypeDefKind::Enum(_) => false,
            TypeDefKind::Option(ty) => has_non_canonical_list_rust2(resolve, ty),
            TypeDefKind::Result(res) => {
                res.ok
                    .as_ref()
                    .map_or(false, |ty| has_non_canonical_list_rust2(resolve, ty))
                    || res
                        .err
                        .as_ref()
                        .map_or(false, |ty| has_non_canonical_list_rust2(resolve, ty))
            }
            TypeDefKind::List(_ty) => true,
            TypeDefKind::Future(_) | TypeDefKind::Stream(_) => false,
            TypeDefKind::Type(ty) => has_non_canonical_list_rust2(resolve, ty),
            TypeDefKind::Unknown => false,
            TypeDefKind::FixedSizeList(ty, _) => has_non_canonical_list_rust2(resolve, ty),
        },
        Type::ErrorContext => todo!(),
    }
}

pub fn has_non_canonical_list_rust(resolve: &Resolve, args: &[(String, Type)]) -> bool {
    args.iter()
        .any(|(_, ty)| has_non_canonical_list_rust2(resolve, ty))
}

fn add_type2(
    resolve: &mut Resolve,
    world: &mut World,
    tp: &Type,
    name: &str,
    iface_map: &mut HashMap<Option<InterfaceId>, InterfaceId>,
) {
    match tp {
        Type::Id(id) => add_type(resolve, world, *id, name, iface_map),
        _ => (),
    }
}

fn add_type(
    resolve: &mut Resolve,
    world: &mut World,
    id: wit_parser::TypeId,
    name: &str,
    iface_map: &mut HashMap<Option<InterfaceId>, InterfaceId>,
) {
    let tp: &TypeDef = &resolve.types[id];
    if tp.name.is_none() {
        return;
    }
    // dbg!(id, tp);
    let old_owner = if let TypeOwner::Interface(owner) = &tp.owner {
        Some(*owner)
    } else {
        None
    };
    let iface = if let Some(new_iface) = iface_map.get(&old_owner) {
        *new_iface
    } else {
        let old_interface: &Interface = &resolve.interfaces[old_owner.unwrap()];
        let name = old_interface.name.clone();
        let iface = Interface {
            name: name.clone(),
            types: Default::default(),
            functions: Default::default(),
            docs: Default::default(),
            stability: Default::default(),
            package: old_interface.package,
        };
        let new_id = resolve.interfaces.alloc(iface);
        iface_map.insert(old_owner, new_id);
        world.imports.insert(
            WorldKey::Name(name.unwrap()),
            WorldItem::Interface {
                id: new_id,
                stability: Default::default(),
            },
        );
        new_id
    };
    let interface = &resolve.interfaces[iface];
    if interface
        .types
        .iter()
        .find(|(_n, id2)| id == **id2)
        .is_none()
    {
        let tp = &mut resolve.types[id];
        tp.owner = TypeOwner::Interface(iface);
        let kind = tp.kind.clone();
        match kind {
            TypeDefKind::Record(record) => {
                for f in record.fields.iter() {
                    add_type2(resolve, world, &f.ty, &f.name, iface_map);
                }
            }
            TypeDefKind::Resource => (),
            TypeDefKind::Handle(handle) => match handle {
                wit_parser::Handle::Own(id) => add_type(resolve, world, id, name, iface_map),
                wit_parser::Handle::Borrow(id) => add_type(resolve, world, id, name, iface_map),
            },
            TypeDefKind::Flags(_flags) => (),
            TypeDefKind::Tuple(tuple) => {
                for (n, tp) in tuple.types.iter().enumerate() {
                    add_type2(resolve, world, tp, &format!("{name}-f{n}"), iface_map);
                }
            }
            TypeDefKind::Variant(variant) => {
                for c in variant.cases.iter() {
                    if let Some(tp) = &c.ty {
                        add_type2(
                            resolve,
                            world,
                            &tp,
                            &format!("{name}-f{}", c.name),
                            iface_map,
                        );
                    }
                }
            }
            TypeDefKind::Enum(_en) => (),
            TypeDefKind::Option(tp) => add_type2(resolve, world, &tp, name, iface_map),
            TypeDefKind::Result(result) => {
                if let Some(tp) = &result.ok {
                    add_type2(resolve, world, tp, &(name.to_string() + "-ok"), iface_map);
                }
                if let Some(tp) = &result.err {
                    add_type2(resolve, world, tp, &(name.to_string() + "-err"), iface_map);
                }
            }
            TypeDefKind::List(tp) => add_type2(resolve, world, &tp, name, iface_map),
            TypeDefKind::FixedSizeList(tp, _sz) => add_type2(resolve, world, &tp, name, iface_map),
            TypeDefKind::Future(tp) => {
                if let Some(tp) = tp {
                    add_type2(resolve, world, &tp, name, iface_map);
                }
            }
            TypeDefKind::Stream(tp) => {
                if let Some(tp) = tp {
                    add_type2(resolve, world, &tp, name, iface_map);
                }
            }
            TypeDefKind::Type(tp) => add_type2(resolve, world, &tp, name, iface_map),
            TypeDefKind::Unknown => todo!(),
        }
        let interface = &mut resolve.interfaces[iface];
        interface.types.insert(name.into(), id);
    }
}

pub fn hash(resolve: &Resolve, func: &wit_parser::Function) -> u64 {
    // dbg!(&func);
    let mut resolve2 = resolve.clone();
    let mut world = wit_parser::World {
        name: "world".into(),
        imports: Default::default(),
        exports: Default::default(),
        package: None,
        docs: Default::default(),
        stability: Default::default(),
        includes: Vec::default(),
        include_names: Vec::default(),
    };
    let interface = Interface {
        name: None,
        types: Default::default(),
        functions: Default::default(),
        docs: Default::default(),
        stability: Default::default(),
        package: Default::default(),
    };
    let iface_id = resolve2.interfaces.alloc(interface);
    world.package = Some(resolve2.packages.alloc(Package {
        name: PackageName {
            namespace: "root".into(),
            name: "root".into(),
            version: None,
        },
        docs: Default::default(),
        interfaces: Default::default(),
        worlds: Default::default(),
    }));
    let mut iface_map: HashMap<Option<InterfaceId>, InterfaceId> = HashMap::new();
    iface_map.insert(None, iface_id);
    for (name, tp) in func.params.iter() {
        add_type2(&mut resolve2, &mut world, tp, name, &mut iface_map);
    }
    if let Some(tp) = &func.result {
        add_type2(&mut resolve2, &mut world, tp, "result", &mut iface_map);
    }
    if !resolve2.interfaces.get(iface_id).unwrap().types.is_empty() {
        world.imports.insert(
            WorldKey::Name("dependencies".into()),
            WorldItem::Interface {
                id: iface_id,
                stability: Default::default(),
            },
        );
    }
    world.imports.insert(
        WorldKey::Name(func.name.clone()),
        WorldItem::Function(func.clone()),
    );
    let world_id = resolve2.worlds.alloc(world);

    let component_type = wit_component::metadata::encode(
        &resolve2,
        world_id,
        wit_component::StringEncoding::UTF8,
        None,
    )
    .unwrap();
    let mut hasher = rustc_stable_hash::hashers::SipHasher128::new_with_keys(0, 1);
    use std::hash::Hasher;
    hasher.write(&component_type);
    let hash = hasher.finish().0[0];
    let parsed = wit_parser::decoding::decode(&component_type);
    if let Ok(DecodedWasm::WitPackage(resolve3, pkg_id)) = parsed {
        let mut wit_printer = wit_component::WitPrinter::default();
        wit_printer.print(&resolve3, pkg_id, &[]).unwrap();
        print!("{hash:x} {}", wit_printer.output.to_string());
    }
    hash
}
