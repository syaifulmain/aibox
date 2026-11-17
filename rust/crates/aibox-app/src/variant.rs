use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QStringList, QVariant};
use serde::Serialize;
use serde_json::{Map, Value};

pub fn json_to_qvariant(value: &Value) -> QVariant {
    match value {
        Value::Null => QVariant::default(),
        Value::Bool(b) => QVariant::from(b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if let Ok(i32_val) = i32::try_from(i) {
                    QVariant::from(&i32_val)
                } else {
                    QVariant::from(&i)
                }
            } else if let Some(u) = n.as_u64() {
                if let Ok(i32_val) = i32::try_from(u) {
                    QVariant::from(&i32_val)
                } else if let Ok(i64_val) = i64::try_from(u) {
                    QVariant::from(&i64_val)
                } else {
                    QVariant::from(&u)
                }
            } else if let Some(f) = n.as_f64() {
                QVariant::from(&f)
            } else {
                QVariant::default()
            }
        }
        Value::String(s) => QVariant::from(&QString::from(s.as_str())),
        Value::Array(arr) => QVariant::from(&json_array_to_qvariant_list(arr)),
        Value::Object(map) => QVariant::from(&json_object_to_qvariant_map(map)),
    }
}

pub fn json_array_to_qvariant_list(arr: &[Value]) -> QList<QVariant> {
    let mut list = QList::<QVariant>::default();
    for item in arr {
        list.append(json_to_qvariant(item));
    }
    list
}

pub fn json_object_to_qvariant_map(map: &Map<String, Value>) -> QMap<QMapPair_QString_QVariant> {
    let mut out = QMap::<QMapPair_QString_QVariant>::default();
    for (k, v) in map {
        out.insert(QString::from(k.as_str()), json_to_qvariant(v));
    }
    out
}

pub fn value_to_qvariant_list(value: &Value) -> QList<QVariant> {
    match value {
        Value::Array(arr) => json_array_to_qvariant_list(arr),
        _ => QList::<QVariant>::default(),
    }
}

pub fn value_to_qvariant_map(value: &Value) -> QMap<QMapPair_QString_QVariant> {
    match value {
        Value::Object(map) => json_object_to_qvariant_map(map),
        _ => QMap::<QMapPair_QString_QVariant>::default(),
    }
}

pub fn serialize_to_qvariant_list<T: Serialize>(items: &[T]) -> QList<QVariant> {
    match serde_json::to_value(items) {
        Ok(val) => value_to_qvariant_list(&val),
        Err(_) => QList::<QVariant>::default(),
    }
}

pub fn serialize_to_qvariant_map<T: Serialize>(item: &T) -> QMap<QMapPair_QString_QVariant> {
    match serde_json::to_value(item) {
        Ok(val) => value_to_qvariant_map(&val),
        Err(_) => QMap::<QMapPair_QString_QVariant>::default(),
    }
}

pub fn strings_to_qstringlist(items: &[String]) -> QStringList {
    let mut qlist = QList::<QString>::default();
    for item in items {
        qlist.append(QString::from(item.as_str()));
    }
    QStringList::from(&qlist)
}

pub fn i32s_to_qvariant_list(items: &[i32]) -> QList<QVariant> {
    let mut list = QList::<QVariant>::default();
    for item in items {
        list.append(QVariant::from(item));
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_to_qvariant_preserves_i32_and_i64() {
        let small = json_to_qvariant(&json!(42));
        assert_eq!(small.value::<i32>(), Some(42));

        let big_val: i64 = 8_804_682_956;
        let big = json_to_qvariant(&json!(big_val));
        assert_eq!(big.value::<i64>(), Some(big_val));

        let text = json_to_qvariant(&json!("ubuntu"));
        assert_eq!(text.value::<QString>(), Some(QString::from("ubuntu")));

        let flag = json_to_qvariant(&json!(true));
        assert_eq!(flag.value::<bool>(), Some(true));
    }
}
