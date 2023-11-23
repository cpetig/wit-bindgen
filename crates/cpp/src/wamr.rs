use wit_bindgen_core::wit_parser::{Function, Resolve, Results, Type, TypeDefKind};

#[derive(Debug, Default)]
pub struct WamrSig {
    wamr_types: String,
    wamr_result: String,
}

fn push_wamr(ty: &Type, resolve: &Resolve, params_str: &mut String) {
    match ty {
        Type::Bool
        | Type::U8
        | Type::U16
        | Type::U32
        | Type::S8
        | Type::S16
        | Type::S32
        | Type::Char => {
            params_str.push('i');
        }
        Type::U64 | Type::S64 => {
            params_str.push('I');
        }
        Type::Float32 => {
            params_str.push('f');
        }
        Type::Float64 => {
            params_str.push('F');
        }
        Type::String => {
            params_str.push_str("$~");
        }
        Type::Id(id) => match &resolve.types[*id].kind {
            TypeDefKind::Type(t) => push_wamr(t, resolve, params_str),
            TypeDefKind::Record(_r) => {
                todo!();
            }
            TypeDefKind::Flags(_) => todo!(),
            TypeDefKind::Tuple(_) => todo!(),
            TypeDefKind::Variant(_) => todo!(),
            TypeDefKind::Enum(_e) => {
                params_str.push_str("i");
            }
            TypeDefKind::Option(_) => todo!(),
            TypeDefKind::Result(_) => todo!(),
            TypeDefKind::List(_t) => {
                params_str.push_str("*~");
            }
            TypeDefKind::Future(_) => todo!(),
            TypeDefKind::Stream(_) => todo!(),
            TypeDefKind::Unknown => todo!(),
            TypeDefKind::Resource => todo!(),
            TypeDefKind::Handle(_h) => {
                params_str.push('i');
            }
        },
    }
}

fn wamr_add_result(sig: &mut WamrSig, resolve: &Resolve, ty: &Type) {
    match ty {
        Type::Bool
        | Type::U8
        | Type::U16
        | Type::U32
        | Type::S8
        | Type::S16
        | Type::S32
        | Type::Char => {
            sig.wamr_result = "i".into();
        }
        Type::S64 | Type::U64 => {
            sig.wamr_result = "I".into();
        }
        Type::Float32 => {
            sig.wamr_result = "f".into();
        }
        Type::Float64 => {
            sig.wamr_result = "F".into();
        }
        Type::String => {
            sig.wamr_types.push('*');
        }
        Type::Id(id) => match &resolve.types[*id].kind {
            TypeDefKind::Record(_) => todo!(),
            TypeDefKind::Flags(_) => todo!(),
            TypeDefKind::Tuple(_) => todo!(),
            TypeDefKind::Variant(_) => todo!(),
            TypeDefKind::Enum(_e) => {
                sig.wamr_types.push('*');
            }
            TypeDefKind::Option(_o) => {
                sig.wamr_types.push('*');
            }
            TypeDefKind::Result(_) => {
                sig.wamr_types.push('*');
            }
            TypeDefKind::List(_) => {
                sig.wamr_types.push('*');
            }
            TypeDefKind::Future(_) => todo!(),
            TypeDefKind::Stream(_) => todo!(),
            TypeDefKind::Type(ty) => wamr_add_result(sig, resolve, &ty),
            TypeDefKind::Unknown => todo!(),
            TypeDefKind::Resource => todo!(),
            TypeDefKind::Handle(_h) => {
                sig.wamr_result = "i".into();
            }
        },
    }
}

pub fn wamr_signature(resolve: &Resolve, func: &Function) -> WamrSig {
    let mut result = WamrSig::default();
    for (_name, param) in func.params.iter() {
        push_wamr(param, resolve, &mut result.wamr_types);
    }
    match &func.results {
        Results::Named(p) => {
            if !p.is_empty() {
                dbg!(p);
                todo!()
            }
        }
        Results::Anon(e) => wamr_add_result(&mut result, resolve, e),
    }
    result
}
