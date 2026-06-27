#[inline]
fn low_u8(value: u32) -> u8 {
    value.to_le_bytes()[0]
}

#[inline]
fn low_u16(value: u32) -> u16 {
    let bytes = value.to_le_bytes();
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn apply_list_item_to_pool(
    pool: &mut ObjectPool,
    list: ObjectID,
    index: usize,
    item: ObjectID,
) -> Result<bool> {
    if item != ObjectID::NULL && pool.find(item).is_none() {
        return Ok(false);
    }
    if pool
        .find(list)
        .is_some_and(|obj| obj.r#type == ObjectType::OutputList)
        && !output_list_item_reference_is_valid(pool, item)
    {
        return Ok(false);
    }
    let Some(obj) = pool.find_mut(list) else {
        return Ok(false);
    };
    match obj.r#type {
        ObjectType::InputList => {
            let mut body = obj.get_input_list_body()?;
            let Some(slot) = body.items.get_mut(index) else {
                return Ok(false);
            };
            if *slot == item {
                return Ok(false);
            }
            *slot = item;
            obj.body = body.encode()?;
            Ok(true)
        }
        ObjectType::OutputList => {
            let mut body = obj.get_output_list_body()?;
            let Some(slot) = body.items.get_mut(index) else {
                return Ok(false);
            };
            if *slot == item {
                return Ok(false);
            }
            *slot = item;
            obj.body = body.encode()?;
            Ok(true)
        }
        ObjectType::ExternalObjectDefinition => {
            let mut body = obj.get_external_object_definition_body()?;
            let Some(slot) = body.object_ids.get_mut(index) else {
                return Ok(false);
            };
            if *slot == item {
                return Ok(false);
            }
            *slot = item;
            obj.body = body.encode()?;
            Ok(true)
        }
        ObjectType::Animation => {
            let Some(slot) = obj.children_pos.get_mut(index) else {
                return Ok(false);
            };
            if slot.id == item {
                return Ok(false);
            }
            slot.id = item;
            obj.children = obj.children_pos.iter().map(|child| child.id).collect();
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn apply_polygon_point_to_pool(
    pool: &mut ObjectPool,
    id: ObjectID,
    index: usize,
    x: u16,
    y: u16,
) -> Result<bool> {
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    if obj.r#type != ObjectType::Polygon {
        return Ok(false);
    }
    let mut body = obj.get_output_polygon_body()?;
    let Some(point) = body.points.get_mut(index) else {
        return Ok(false);
    };
    if point.x == x && point.y == y {
        return Ok(false);
    }
    *point = PolygonPoint { x, y };
    obj.body = body.encode()?;
    Ok(true)
}

fn apply_polygon_scale_to_pool(
    pool: &mut ObjectPool,
    id: ObjectID,
    new_width: u16,
    new_height: u16,
) -> Result<bool> {
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    if obj.r#type != ObjectType::Polygon {
        return Ok(false);
    }
    let mut body = obj.get_output_polygon_body()?;
    if body.width == new_width && body.height == new_height {
        return Ok(false);
    }
    if body.width == 0 || body.height == 0 {
        return Err(Error::invalid_state(
            "VT render runtime cannot scale a polygon with zero width or height",
        ));
    }
    let old_width = u32::from(body.width);
    let old_height = u32::from(body.height);
    let next_width = u32::from(new_width);
    let next_height = u32::from(new_height);
    for point in &mut body.points {
        point.x = scale_polygon_coordinate(point.x, next_width, old_width);
        point.y = scale_polygon_coordinate(point.y, next_height, old_height);
    }
    body.width = new_width;
    body.height = new_height;
    obj.body = body.encode()?;
    Ok(true)
}

fn scale_polygon_coordinate(old: u16, new_extent: u32, old_extent: u32) -> u16 {
    (((u32::from(old) * new_extent) + (old_extent / 2)) / old_extent).min(u32::from(u16::MAX))
        as u16
}

#[inline]
fn line_art_bit(line_art: u16, step: usize) -> bool {
    let bit = 15usize.saturating_sub(step % 16);
    (line_art & (1u16 << bit)) != 0
}
