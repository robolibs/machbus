/// Apply a macro Change Numeric Value using the same retained object-state
/// targets as the hosted runtime path. Variable-backed objects update the
/// referenced Number Variable; inline value objects update their own body
/// fields after width/type/reference validation.
fn apply_numeric_value(pool: &mut ObjectPool, target: ObjectID, value: u32) -> bool {
    if !macro_numeric_value_update_is_valid(pool, target, value) {
        return false;
    }
    if pool.set_number_variable_value(target, value) {
        return true;
    }
    if let Some(variable_reference) = pool
        .find(target)
        .and_then(|object| match object.r#type {
            ObjectType::OutputNumber => object
                .get_output_number_body()
                .ok()
                .map(|body| body.variable_reference),
            ObjectType::InputNumber => object
                .get_input_number_body()
                .ok()
                .map(|body| body.variable_reference),
            _ => None,
        })
        .filter(|variable_reference| *variable_reference != ObjectID::NULL)
    {
        return pool.set_number_variable_value(variable_reference, value);
    }
    if let Some(body) = pool
        .find(target)
        .filter(|obj| obj.r#type == ObjectType::InputList)
        .and_then(|obj| obj.get_input_list_body().ok())
        .filter(|body| body.variable_reference != ObjectID::NULL)
    {
        return pool.set_number_variable_value(body.variable_reference, value);
    }
    let Some(obj) = pool.find_mut(target) else {
        return false;
    };
    match obj.r#type {
        ObjectType::OutputNumber => {
            let Ok(mut body) = obj.get_output_number_body() else {
                return false;
            };
            if body.variable_reference != ObjectID::NULL {
                return false;
            }
            body.value = value;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::InputNumber => {
            let Ok(mut body) = obj.get_input_number_body() else {
                return false;
            };
            if body.variable_reference != ObjectID::NULL {
                return false;
            }
            body.value = value;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::InputBoolean => {
            let Ok(mut body) = obj.get_input_boolean_body() else {
                return false;
            };
            if body.variable_reference != ObjectID::NULL {
                return false;
            }
            body.value = u8::from(value != 0);
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::InputList => {
            let Ok(mut body) = obj.get_input_list_body() else {
                return false;
            };
            if body.variable_reference != ObjectID::NULL {
                return false;
            }
            body.value = value as u8;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::OutputList => {
            let Ok(mut body) = obj.get_output_list_body() else {
                return false;
            };
            if body.variable_reference != ObjectID::NULL {
                return false;
            }
            body.value = value as u8;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::ObjectPointer => {
            let Ok(mut body) = obj.get_object_pointer_body() else {
                return false;
            };
            body.value = ObjectID(value as u16);
            obj.body = body.encode();
            true
        }
        ObjectType::ExternalObjectPointer => {
            let Ok(mut body) = obj.get_external_object_pointer_body() else {
                return false;
            };
            body.external_reference_name = ObjectID(value as u16);
            body.external_object_id = ObjectID((value >> 16) as u16);
            obj.body = body.encode();
            true
        }
        ObjectType::ScaledGraphic => {
            let Ok(mut body) = obj.get_scaled_graphic_body() else {
                return false;
            };
            body.value = ObjectID(value as u16);
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::Animation => {
            let Ok(mut body) = obj.get_animation_body() else {
                return false;
            };
            body.value = value as u8;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::Meter => {
            let Ok(mut body) = obj.get_meter_body() else {
                return false;
            };
            if body.variable_reference != ObjectID::NULL {
                return false;
            }
            body.value = value as u16;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::LinearBarGraph => {
            let Ok(mut body) = obj.get_linear_bar_graph_body() else {
                return false;
            };
            if body.variable_reference != ObjectID::NULL {
                return false;
            }
            body.value = value as u16;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::ArchedBarGraph => {
            let Ok(mut body) = obj.get_arched_bar_graph_body() else {
                return false;
            };
            if body.variable_reference != ObjectID::NULL {
                return false;
            }
            body.value = value as u16;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        _ => false,
    }
}

fn macro_numeric_value_update_is_valid(pool: &ObjectPool, target: ObjectID, value: u32) -> bool {
    let Some(object) = pool.find(target) else {
        return false;
    };
    if !macro_numeric_value_fits_object_width(object.r#type, value) {
        return false;
    }
    match object.r#type {
        ObjectType::InputBoolean => value <= 1,
        ObjectType::Animation => {
            value <= u32::from(u8::MAX)
                && (value == u32::from(u8::MAX) || (value as usize) < object.children_pos.len())
        }
        ObjectType::ObjectPointer => {
            object_pointer_numeric_value_is_valid_for_context(pool, target, ObjectID(value as u16))
        }
        ObjectType::ScaledGraphic => {
            scaled_graphic_value_source_is_valid(pool, ObjectID(value as u16))
        }
        ObjectType::ExternalObjectPointer => {
            let reference_name = ObjectID(value as u16);
            reference_name == ObjectID::NULL
                || pool
                    .find(reference_name)
                    .is_some_and(|obj| obj.r#type == ObjectType::ExternalReferenceName)
        }
        _ => true,
    }
}

fn macro_numeric_value_fits_object_width(object_type: ObjectType, value: u32) -> bool {
    match object_type {
        ObjectType::InputBoolean
        | ObjectType::InputList
        | ObjectType::OutputList
        | ObjectType::Animation => value <= u32::from(u8::MAX),
        ObjectType::Meter
        | ObjectType::LinearBarGraph
        | ObjectType::ArchedBarGraph
        | ObjectType::ObjectPointer
        | ObjectType::ScaledGraphic => value <= u32::from(u16::MAX),
        ObjectType::ExternalObjectPointer
        | ObjectType::InputNumber
        | ObjectType::OutputNumber
        | ObjectType::NumberVariable => true,
        _ => false,
    }
}

/// Apply a macro Change String Value using the same fixed-length string targets
/// as the server/runtime command path: String Variable, inline Output String,
/// and Input Attributes validation string.
fn apply_string_value(pool: &mut ObjectPool, target: ObjectID, value: Vec<u8>) -> bool {
    let Ok(text) = core::str::from_utf8(&value) else {
        return false;
    };
    if pool.set_string_variable_value(target, value.clone()) {
        return true;
    }
    let Some(obj) = pool.find_mut(target) else {
        return false;
    };
    match obj.r#type {
        ObjectType::OutputString => {
            let Ok(mut body) = obj.get_output_string_body() else {
                return false;
            };
            if body.variable_reference != ObjectID::NULL {
                return false;
            }
            let max_len = body.value.len();
            if text.len() > max_len {
                return false;
            }
            body.value = padded_macro_string_bytes(text, max_len);
            let Ok(encoded) = body.encode() else {
                return false;
            };
            if obj.body == encoded {
                false
            } else {
                obj.body = encoded;
                true
            }
        }
        ObjectType::InputAttributes => {
            let Ok(mut body) = obj.get_input_attributes_body() else {
                return false;
            };
            let max_len = body.validation_string.len();
            if text.len() > max_len {
                return false;
            }
            body.validation_string = padded_macro_string_bytes(text, max_len);
            let Ok(encoded) = body.encode() else {
                return false;
            };
            if obj.body == encoded {
                false
            } else {
                obj.body = encoded;
                true
            }
        }
        _ => false,
    }
}

fn padded_macro_string_bytes(text: &str, max_len: usize) -> Vec<u8> {
    let mut value = text.as_bytes().to_vec();
    value.resize(max_len, b' ');
    value
}

/// Index from `(object id, event id)` to the Macro object ids bound to
/// that event on that object.
///
/// `event_id` is kept as the raw ISO 11783-6 macro-event byte; callers
/// map their own operator/protocol events onto those byte values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MacroTriggerIndex {
    triggers: HashMap<(ObjectID, u8), Vec<ObjectID>>,
}

impl MacroTriggerIndex {
    /// Build the index by walking every object's macro reference list.
    /// Classic 8-bit macro ids are widened to the Macro object's id.
    #[must_use]
    pub fn build(pool: &ObjectPool) -> Self {
        let mut triggers: HashMap<(ObjectID, u8), Vec<ObjectID>> = HashMap::new();
        for obj in pool.objects() {
            for mref in &obj.macros {
                triggers
                    .entry((obj.id, mref.event_id))
                    .or_default()
                    .push(ObjectID::new(u16::from(mref.macro_id)));
            }
        }
        Self { triggers }
    }

    /// Macro object ids to run when `event_id` fires on `object`.
    #[must_use]
    pub fn macros_for(&self, object: ObjectID, event_id: u8) -> &[ObjectID] {
        self.triggers
            .get(&(object, event_id))
            .map_or(&[], Vec::as_slice)
    }

    /// `true` if no object in the pool binds any macro.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.triggers.is_empty()
    }

    /// Number of distinct `(object, event)` bindings.
    #[must_use]
    pub fn binding_count(&self) -> usize {
        self.triggers.len()
    }
}

