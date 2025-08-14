#[macro_export]
macro_rules! first_node {
    ($ast:expr, $type:path) => {{
        let mut check: Vec<&markdown::mdast::Node> = vec![$ast];
        let mut value = None;

        while check.len() != 0 {
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

                    check.remove(0);
                }
            }
        }

        value
    }};
}

#[macro_export]
macro_rules! first_within_child {
    ($index:expr, $ast:expr, $type:path) => {{
        $ast.children
            .get($index)
            .map(|child| first_node!(child, $type))
            .flatten()
    }};
}
