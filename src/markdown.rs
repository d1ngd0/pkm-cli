#[macro_export]
macro_rules! find_node {
    ($ast:expr, $type:path) => {{
        let mut check: Vec<&markdown::mdast::Node> = vec![$ast];
        let mut value = None;

        while check.len() != 0 {
            dbg!(check[0]);
            match check[0] {
                $type(v) => {
                    value = Some(v);
                    check.clear();
                }
                _ => {
                    if let Some(children) = check[0].children() {
                        for child in children {
                            check.push(child)
                        }
                    }
                }
            }

            check.remove(0);
        }

        value
    }};
}
