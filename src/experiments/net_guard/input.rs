pub(super) use crate::python_json::{Node, Text, parse};

impl Text {
    pub(super) fn record(&self) -> String {
        format!("{{\"command\": {}}}\n", self.json_prefix(400))
    }
}
impl Node {
    pub(super) fn as_command(&self) -> Text {
        match self {
            Self::String(v) => v.clone(),
            Self::List(v) => {
                let mut out = Vec::new();
                for (i, v) in v.iter().enumerate() {
                    if i > 0 {
                        out.push(32);
                    }
                    out.extend(v.scalar().0);
                }
                Text(out)
            }
            _ => Text::default(),
        }
    }
    pub(super) fn find_command(&self, depth: i64) -> Text {
        if depth < 0 {
            return Text::default();
        }
        match self {
            Self::Dict(items) => {
                if let Some(v) = self.field("command") {
                    let got = v.as_command();
                    if !got.0.is_empty() {
                        return got;
                    }
                }
                for value in items.values() {
                    let got = value.find_command(depth - 1);
                    if !got.0.is_empty() {
                        return got;
                    }
                }
            }
            Self::List(items) => {
                for value in items {
                    let got = value.find_command(depth - 1);
                    if !got.0.is_empty() {
                        return got;
                    }
                }
            }
            _ => (),
        }
        Text::default()
    }
    pub(super) fn hook_command(&self) -> Text {
        if !matches!(self, Self::Dict(_)) {
            return Text::default();
        }
        for key in ["tool_input", "toolArgs", "input", "arguments", "params"] {
            if let Some(args) = self.field(key)
                && let Some(command) = args.field("command")
                && command.truthy()
            {
                return command.as_command();
            }
        }
        if let Some(command) = self.field("command")
            && command.truthy()
        {
            return command.as_command();
        }
        self.find_command(4)
    }
}
pub(super) fn command(bytes: &[u8], int_max_digits: usize) -> Option<Text> {
    parse(bytes, int_max_digits).map(|v| v.hook_command())
}
