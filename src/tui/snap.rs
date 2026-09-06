use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub fn frame_to_string(buffer: &Buffer, area: Rect) -> String {
    let width = buffer.area.width as usize;
    let mut lines = Vec::new();
    for y in area.top()..area.bottom() {
        let start = (y - buffer.area.y) as usize * width + (area.x - buffer.area.x) as usize;
        let cells = &buffer.content[start..start + area.width as usize];
        let line: String = cells.iter().map(|c| c.symbol()).collect();
        lines.push(line.trim_end().to_string());
    }
    lines.join("\n")
}
