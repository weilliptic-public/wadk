use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

use crate::{State, StateA, StateB, StateC, StateD, StateE, Step};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateEnum {
    StateA,
    StateB,
    StateC,
    StateD,
    StateE,
}

impl StateEnum {
    /// Converts a StateEnum variant into a boxed State trait object
    /// 
    /// # Returns
    /// A boxed State trait object corresponding to the enum variant
    pub fn to_boxed_state(&self) -> Box<dyn State> {
        match self {
            StateEnum::StateA => Box::new(StateA),
            StateEnum::StateB => Box::new(StateB),
            StateEnum::StateC => Box::new(StateC),
            StateEnum::StateD => Box::new(StateD),
            StateEnum::StateE => Box::new(StateE),
        }
    }
}

pub mod handlers_serde {
    use super::*;

    /// Serializes a BTreeMap of State handlers by converting them to StateEnum variants
    /// 
    /// # Arguments
    /// * `handlers` - The BTreeMap of Step to State handlers
    /// * `serializer` - The serializer to use
    pub fn serialize<S>(
        handlers: &BTreeMap<Step, Box<dyn State>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = BTreeMap::new();
        for (step, _) in handlers {
            let state_enum = match step {
                Step::A => StateEnum::StateA,
                Step::B => StateEnum::StateB,
                Step::C => StateEnum::StateC,
                Step::D => StateEnum::StateD,
                Step::E => StateEnum::StateE,
            };
            map.insert(step.clone(), state_enum);
        }
        map.serialize(serializer)
    }

    /// Deserializes StateEnum variants back into a BTreeMap of State handlers
    /// 
    /// # Arguments
    /// * `deserializer` - The deserializer to use
    /// 
    /// # Returns
    /// A BTreeMap of Step to State handlers
    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<Step, Box<dyn State>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map: BTreeMap<Step, StateEnum> = BTreeMap::deserialize(deserializer)?;
        let mut handlers = BTreeMap::new();
        for (step, state_enum) in map {
            let boxed_state = state_enum.to_boxed_state();
            handlers.insert(step, boxed_state);
        }
        Ok(handlers)
    }
}
