//! General-purpose utilities for deriving structqlx

/// Return true if a given type is an Option<>, false otherwise.
pub(crate) fn is_option(type_: &syn::Type) -> bool {
    let syn::Type::Path(type_) = type_ else {
        return false;
    };
    if type_.qself.is_some() {
        return false;
    }
    let type_path = &type_.path;
    if type_path.segments.is_empty() || type_path.segments.last().unwrap().ident != "Option" {
        return false;
    }

    if !(type_path.segments.len() == 1
        || (type_path.segments.len() == 3
            && ["core", "std"].contains(&type_path.segments[0].ident.to_string().as_str())
            && type_path.segments[1].ident == "option"))
    {
        return false;
    }

    true
}

/// Walk the path of a given type. If it's an Option<*> field, return the inner
/// type, otherwise return None.
#[allow(unused)]
pub(crate) fn get_field_type_from_option(type_: &syn::Type) -> Option<&syn::Type> {
    if !is_option(type_) {
        return None;
    }

    let syn::Type::Path(type_) = type_ else {
        return None;
    };
    let type_path = &type_.path;
    let last_segment = type_path.segments.last().unwrap();
    let syn::PathArguments::AngleBracketed(generics) = &last_segment.arguments else {
        return None;
    };
    if generics.args.len() != 1 {
        return None;
    }
    let syn::GenericArgument::Type(inner_type) = &generics.args[0] else {
        return None;
    };

    Some(inner_type)
}
