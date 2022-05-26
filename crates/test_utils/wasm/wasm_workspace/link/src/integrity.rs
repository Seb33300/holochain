use std::array::IntoIter;
use std::ops::RangeBounds;

use holochain_deterministic_integrity::prelude::*;

#[hdk_to_global_link_types]
#[hdk_to_local_types]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum LinkTypes {
    SomeLinks,
    SomeOtherLinks,
}

impl TryFrom<LinkTypes> for LinkType {
    type Error = WasmError;

    fn try_from(value: LinkTypes) -> Result<Self, Self::Error> {
        Ok(Self(GlobalZomeTypeId::try_from(value)?.0))
    }
}

impl TryFrom<&LinkTypes> for LinkType {
    type Error = WasmError;

    fn try_from(value: &LinkTypes) -> Result<Self, Self::Error> {
        Ok(Self(GlobalZomeTypeId::try_from(value)?.0))
    }
}

impl TryFrom<LinkTypes> for LinkTypeRange {
    type Error = WasmError;

    fn try_from(value: LinkTypes) -> Result<Self, Self::Error> {
        let lt: LinkType = value.try_into()?;
        Ok(lt.into())
    }
}

impl TryFrom<&LinkTypes> for LinkTypeRange {
    type Error = WasmError;

    fn try_from(value: &LinkTypes) -> Result<Self, Self::Error> {
        let lt: LinkType = value.try_into()?;
        Ok(lt.into())
    }
}

impl TryFrom<LinkTypes> for LinkTypeRanges {
    type Error = WasmError;

    fn try_from(value: LinkTypes) -> Result<Self, Self::Error> {
        let lt: LinkType = value.try_into()?;
        Ok(Self(vec![lt.into()]))
    }
}

impl TryFrom<&LinkTypes> for LinkTypeRanges {
    type Error = WasmError;

    fn try_from(value: &LinkTypes) -> Result<Self, Self::Error> {
        let lt: LinkType = value.try_into()?;
        Ok(Self(vec![lt.into()]))
    }
}

pub trait LinkTypesHelper<const L: u8, const LEN: usize>: EnumLen<L>
where
    Self: Into<LocalZomeTypeId>,
    Self: Clone + Copy + Sized + PartialEq + PartialOrd + 'static,
{
    fn range(
        range: impl RangeBounds<Self> + 'static,
    ) -> Box<dyn FnOnce() -> Result<LinkTypeRange, WasmError>> {
        let zome_types = zome_info().map(|t| t.zome_types);
        let f = move || {
            let zome_types = zome_types?;

            let start = Self::iter().find(|t| range.contains(t));
            match start {
                Some(start) => {
                    let end = Self::iter()
                        .rev()
                        .find(|t| range.contains(t))
                        .unwrap_or(start);
                    let start = zome_types.links.to_global_scope(start).unwrap();
                    let end = zome_types.links.to_global_scope(end).unwrap();
                    Ok(LinkTypeRange::Inclusive(
                        LinkType::from(start)..=LinkType::from(end),
                    ))
                }
                None => Ok(LinkTypeRange::Empty),
            }
        };
        Box::new(f)
    }

    fn iter() -> IntoIter<Self, LEN>;
}

impl LinkTypesHelper<{ LinkTypes::len() }, { LinkTypes::len() as usize }> for LinkTypes {
    fn iter() -> IntoIter<Self, { LinkTypes::len() as usize }> {
        [Self::SomeLinks, Self::SomeOtherLinks].into_iter()
    }
}

// impl Index<RangeFull> for LinkTypes {
//     type Output = LinkTypeRange;

//     fn index(&self, _: RangeFull) -> &Self::Output {
//         &LinkTypeRange::Full
//     }
// }
