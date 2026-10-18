//! Exhaustive tests for chart kind identity and round-trip fidelity across all supported chart types.

use crate::{read_xlsx, write_xlsx};
use gridcraft_core::{CellRef, Value};
use gridcraft_model::*;
use std::sync::Arc;

fn at(a: &str) -> CellRef {
    CellRef::parse(a).expect("valid cell ref")
}

fn make_base_sheet() -> Sheet {
    let mut s = Sheet::new("Data");
    s.set_value(at("A1"), Value::from("Category"));
    s.set_value(at("B1"), Value::from("Metric A"));
    s.set_value(at("C1"), Value::from("Metric B"));
    s.set_value(at("D1"), Value::from("Metric C"));
    s.set_value(at("E1"), Value::from("Metric D"));

    for i in 2..=6 {
        s.set_value(CellRef::new(i, 1), Value::from(format!("Item {}", i - 1)));
        s.set_value(CellRef::new(i, 2), Value::Number((i * 10) as f64));
        s.set_value(CellRef::new(i, 3), Value::Number((i * 15) as f64));
        s.set_value(CellRef::new(i, 4), Value::Number((i * 5) as f64));
        s.set_value(CellRef::new(i, 5), Value::Number((i * 20) as f64));
    }
    s
}

fn sample_series(name: &str, values: &str, categories: &str) -> Series {
    Series {
        name: Some(name.to_string()),
        categories: Some(categories.to_string()),
        values: values.to_string(),
        color: None,
        secondary: false,
        kind: None,
        smooth: false,
        trendlines: Vec::new(),
        bubble_sizes: None,
    }
}

fn sample_anchor() -> Anchor {
    Anchor { cell: at("G2"), dx: 0.0, dy: 0.0, width: 480.0, height: 320.0, mode: AnchorMode::MoveAndSize }
}

fn roundtrip_chart(sheet: Sheet, chart: Chart) -> (Chart, Vec<u8>) {
    let mut wb = Workbook::new();
    let mut s = sheet;
    s.charts.push(chart);
    wb.sheets = vec![Arc::new(s)];

    let bytes = write_xlsx(&wb).expect("write xlsx");
    let (read_wb, report) = read_xlsx(&bytes).expect("read xlsx");
    assert!(report.warnings.is_empty(), "unexpected warnings: {:?}", report.warnings);

    let read_sheet = read_wb.sheet(0).expect("sheet 0");
    assert_eq!(read_sheet.charts.len(), 1, "expected exactly 1 chart back");
    (read_sheet.charts[0].clone(), bytes)
}

#[test]
fn all_standard_chart_kinds_roundtrip() {
    let kinds = [
        ChartKind::ColumnClustered,
        ChartKind::ColumnStacked,
        ChartKind::ColumnStacked100,
        ChartKind::BarClustered,
        ChartKind::BarStacked,
        ChartKind::BarStacked100,
        ChartKind::Line,
        ChartKind::LineMarkers,
        ChartKind::LineStacked,
        ChartKind::Pie,
        ChartKind::Doughnut,
        ChartKind::Area,
        ChartKind::AreaStacked,
        ChartKind::Scatter,
        ChartKind::ScatterLines,
        ChartKind::Bubble,
        ChartKind::Radar,
    ];

    for kind in kinds {
        let sheet = make_base_sheet();
        let chart = Chart {
            id: 1,
            anchor: sample_anchor(),
            kind,
            title: Some(format!("Test {:?}", kind)),
            series: vec![
                sample_series("Metric A", "Data!$B$2:$B$6", "Data!$A$2:$A$6"),
                sample_series("Metric B", "Data!$C$2:$C$6", "Data!$A$2:$A$6"),
            ],
            legend: LegendPos::Right,
            data_labels: true,
            gridlines: true,
            style: 2,
            x_title: Some("Categories".into()),
            y_title: Some("Values".into()),
            source: None,
            by_rows: false,
            pivot: None,
        };

        let (read_back, _) = roundtrip_chart(sheet, chart);
        assert_eq!(read_back.kind, kind, "Kind mismatch for {:?}", kind);
        assert_eq!(read_back.title, Some(format!("Test {:?}", kind)));
        assert_eq!(read_back.series.len(), 2);
        assert_eq!(read_back.series[0].name.as_deref(), Some("Metric A"));
        assert_eq!(read_back.series[0].values, "Data!$B$2:$B$6");
        assert_eq!(read_back.series[0].categories.as_deref(), Some("Data!$A$2:$A$6"));
    }
}

#[test]
fn stock_chart_roundtrip_fidelity() {
    // 1. Stock chart with 2 series (e.g. High, Low) - must NOT demote to Line!
    {
        let sheet = make_base_sheet();
        let chart = Chart {
            id: 10,
            anchor: sample_anchor(),
            kind: ChartKind::Stock,
            title: Some("2-Series Stock".into()),
            series: vec![sample_series("High", "Data!$B$2:$B$6", "Data!$A$2:$A$6"), sample_series("Low", "Data!$C$2:$C$6", "Data!$A$2:$A$6")],
            legend: LegendPos::Top,
            data_labels: false,
            gridlines: true,
            style: 1,
            x_title: None,
            y_title: None,
            source: None,
            by_rows: false,
            pivot: None,
        };
        let (read_back, bytes) = roundtrip_chart(sheet, chart);
        assert_eq!(read_back.kind, ChartKind::Stock, "2-series stock must preserve Stock kind");
        assert_eq!(read_back.series.len(), 2);

        let zip_text = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(zip_text).expect("zip");
        let mut chart_xml = String::new();
        std::io::Read::read_to_string(&mut archive.by_name("xl/charts/chart1.xml").expect("chart1"), &mut chart_xml).expect("read");
        assert!(chart_xml.contains("stockChart"), "DrawingML must contain c:stockChart");
    }

    // 2. Stock chart with 3 series (High-Low-Close)
    {
        let sheet = make_base_sheet();
        let chart = Chart {
            id: 11,
            anchor: sample_anchor(),
            kind: ChartKind::Stock,
            title: Some("3-Series Stock HLC".into()),
            series: vec![
                sample_series("High", "Data!$B$2:$B$6", "Data!$A$2:$A$6"),
                sample_series("Low", "Data!$C$2:$C$6", "Data!$A$2:$A$6"),
                sample_series("Close", "Data!$D$2:$D$6", "Data!$A$2:$A$6"),
            ],
            legend: LegendPos::Right,
            data_labels: false,
            gridlines: true,
            style: 1,
            x_title: None,
            y_title: None,
            source: None,
            by_rows: false,
            pivot: None,
        };
        let (read_back, _) = roundtrip_chart(sheet, chart);
        assert_eq!(read_back.kind, ChartKind::Stock);
        assert_eq!(read_back.series.len(), 3);
    }

    // 3. Stock chart with 4 series (Open-High-Low-Close) - writes upDownBars
    {
        let sheet = make_base_sheet();
        let chart = Chart {
            id: 12,
            anchor: sample_anchor(),
            kind: ChartKind::Stock,
            title: Some("4-Series Stock OHLC".into()),
            series: vec![
                sample_series("Open", "Data!$B$2:$B$6", "Data!$A$2:$A$6"),
                sample_series("High", "Data!$C$2:$C$6", "Data!$A$2:$A$6"),
                sample_series("Low", "Data!$D$2:$D$6", "Data!$A$2:$A$6"),
                sample_series("Close", "Data!$E$2:$E$6", "Data!$A$2:$A$6"),
            ],
            legend: LegendPos::None,
            data_labels: false,
            gridlines: true,
            style: 1,
            x_title: None,
            y_title: None,
            source: None,
            by_rows: false,
            pivot: None,
        };
        let (read_back, bytes) = roundtrip_chart(sheet, chart);
        assert_eq!(read_back.kind, ChartKind::Stock);
        assert_eq!(read_back.series.len(), 4);

        let zip_text = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(zip_text).expect("zip");
        let mut chart_xml = String::new();
        std::io::Read::read_to_string(&mut archive.by_name("xl/charts/chart1.xml").expect("chart1"), &mut chart_xml).expect("read");
        assert!(chart_xml.contains("upDownBars"), "4-series stock chart must contain upDownBars");
    }
}

#[test]
fn pie_of_pie_and_bar_of_pie_roundtrip() {
    // Pie of Pie
    {
        let sheet = make_base_sheet();
        let chart = Chart {
            id: 20,
            anchor: sample_anchor(),
            kind: ChartKind::PieOfPie,
            title: Some("Breakdown Pie of Pie".into()),
            series: vec![sample_series("Share", "Data!$B$2:$B$6", "Data!$A$2:$A$6")],
            legend: LegendPos::Right,
            data_labels: true,
            gridlines: false,
            style: 1,
            x_title: None,
            y_title: None,
            source: None,
            by_rows: false,
            pivot: None,
        };
        let (read_back, bytes) = roundtrip_chart(sheet, chart);
        assert_eq!(read_back.kind, ChartKind::PieOfPie);
        assert_eq!(read_back.title, Some("Breakdown Pie of Pie".into()));

        let zip_text = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(zip_text).expect("zip");
        let mut chart_xml = String::new();
        std::io::Read::read_to_string(&mut archive.by_name("xl/charts/chart1.xml").expect("chart1"), &mut chart_xml).expect("read");
        assert!(chart_xml.contains("ofPieChart"), "Must serialize as c:ofPieChart");
        assert!(chart_xml.contains(r#"ofPieType val="pie""#), "Must specify ofPieType pie");
    }

    // Bar of Pie
    {
        let sheet = make_base_sheet();
        let chart = Chart {
            id: 21,
            anchor: sample_anchor(),
            kind: ChartKind::BarOfPie,
            title: Some("Breakdown Bar of Pie".into()),
            series: vec![sample_series("Share", "Data!$B$2:$B$6", "Data!$A$2:$A$6")],
            legend: LegendPos::Bottom,
            data_labels: true,
            gridlines: false,
            style: 1,
            x_title: None,
            y_title: None,
            source: None,
            by_rows: false,
            pivot: None,
        };
        let (read_back, bytes) = roundtrip_chart(sheet, chart);
        assert_eq!(read_back.kind, ChartKind::BarOfPie);
        assert_eq!(read_back.title, Some("Breakdown Bar of Pie".into()));

        let zip_text = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(zip_text).expect("zip");
        let mut chart_xml = String::new();
        std::io::Read::read_to_string(&mut archive.by_name("xl/charts/chart1.xml").expect("chart1"), &mut chart_xml).expect("read");
        assert!(chart_xml.contains("ofPieChart"), "Must serialize as c:ofPieChart");
        assert!(chart_xml.contains(r#"ofPieType val="bar""#), "Must specify ofPieType bar");
    }
}

#[test]
fn chartex_office_2016_charts_roundtrip() {
    let chartex_kinds =
        [ChartKind::Histogram, ChartKind::Waterfall, ChartKind::Treemap, ChartKind::Sunburst, ChartKind::BoxWhisker, ChartKind::Funnel];

    for kind in chartex_kinds {
        let sheet = make_base_sheet();
        let chart = Chart {
            id: 30,
            anchor: sample_anchor(),
            kind,
            title: Some(format!("Chartex {:?}", kind)),
            series: vec![sample_series("Metric A", "Data!$B$2:$B$6", "Data!$A$2:$A$6")],
            legend: LegendPos::Right,
            data_labels: false,
            gridlines: true,
            style: 1,
            x_title: None,
            y_title: None,
            source: None,
            by_rows: false,
            pivot: None,
        };

        let (read_back, bytes) = roundtrip_chart(sheet, chart);
        assert_eq!(read_back.kind, kind, "Chartex kind mismatch for {:?}", kind);
        assert_eq!(read_back.title, Some(format!("Chartex {:?}", kind)));
        assert_eq!(read_back.series.len(), 1);
        assert_eq!(read_back.series[0].values, "Data!$B$2:$B$6");
        assert_eq!(read_back.series[0].categories.as_deref(), Some("Data!$A$2:$A$6"));

        let zip_text = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(zip_text).expect("zip");
        // Verify chartEx part exists in zip
        let has_chartex = (0..archive.len()).any(|i| {
            let file = archive.by_index(i).expect("file");
            file.name().contains("chartEx")
        });
        assert!(has_chartex, "Zip package must contain a chartEx part for {:?}", kind);
    }
}

#[test]
fn combo_chart_comprehensive_roundtrip() {
    // 1. Multi-type combo with interleaved series kinds and primary/secondary axes
    {
        let sheet = make_base_sheet();
        let combo = Chart {
            id: 40,
            anchor: sample_anchor(),
            kind: ChartKind::Combo,
            title: Some("Revenue & Margins".into()),
            series: vec![
                Series {
                    name: Some("Column Primary".into()),
                    categories: Some("Data!$A$2:$A$6".into()),
                    values: "Data!$B$2:$B$6".into(),
                    color: Some(Color::Rgb(0x1F77B4)),
                    secondary: false,
                    kind: Some(ChartKind::ColumnClustered),
                    smooth: false,
                    trendlines: Vec::new(),
                    bubble_sizes: None,
                },
                Series {
                    name: Some("Line Secondary".into()),
                    categories: Some("Data!$A$2:$A$6".into()),
                    values: "Data!$C$2:$C$6".into(),
                    color: Some(Color::Rgb(0xFF7F0E)),
                    secondary: true,
                    kind: Some(ChartKind::Line),
                    smooth: true,
                    trendlines: Vec::new(),
                    bubble_sizes: None,
                },
                Series {
                    name: Some("Area Primary".into()),
                    categories: Some("Data!$A$2:$A$6".into()),
                    values: "Data!$D$2:$D$6".into(),
                    color: Some(Color::Rgb(0x2CA02C)),
                    secondary: false,
                    kind: Some(ChartKind::Area),
                    smooth: false,
                    trendlines: Vec::new(),
                    bubble_sizes: None,
                },
            ],
            legend: LegendPos::Bottom,
            data_labels: false,
            gridlines: true,
            style: 2,
            x_title: Some("Quarter".into()),
            y_title: Some("USD".into()),
            source: None,
            by_rows: false,
            pivot: None,
        };

        let (read_back, _) = roundtrip_chart(sheet, combo);
        assert_eq!(read_back.kind, ChartKind::Combo, "Combo kind must be preserved");
        assert_eq!(read_back.series.len(), 3);

        // Verify Series 0
        assert_eq!(read_back.series[0].name.as_deref(), Some("Column Primary"));
        assert_eq!(read_back.series[0].kind, Some(ChartKind::ColumnClustered));
        assert!(!read_back.series[0].secondary, "Series 0 should be on primary axis");

        // Verify Series 1
        assert_eq!(read_back.series[1].name.as_deref(), Some("Line Secondary"));
        assert_eq!(read_back.series[1].kind, Some(ChartKind::Line));
        assert!(read_back.series[1].secondary, "Series 1 should be on secondary axis");
        assert!(read_back.series[1].smooth, "Series 1 smooth flag should be preserved");

        // Verify Series 2
        assert_eq!(read_back.series[2].name.as_deref(), Some("Area Primary"));
        assert_eq!(read_back.series[2].kind, Some(ChartKind::Area));
        assert!(!read_back.series[2].secondary, "Series 2 should be on primary axis");
    }

    // 2. Combo chart with single series / uniform series kind
    // Must write dummy secondary group so read_chart preserves ChartKind::Combo
    {
        let sheet = make_base_sheet();
        let combo_single = Chart {
            id: 41,
            anchor: sample_anchor(),
            kind: ChartKind::Combo,
            title: Some("Single Kind Combo".into()),
            series: vec![Series {
                name: Some("Solo Column".into()),
                categories: Some("Data!$A$2:$A$6".into()),
                values: "Data!$B$2:$B$6".into(),
                color: None,
                secondary: false,
                kind: Some(ChartKind::ColumnClustered),
                smooth: false,
                trendlines: Vec::new(),
                bubble_sizes: None,
            }],
            legend: LegendPos::Right,
            data_labels: false,
            gridlines: true,
            style: 1,
            x_title: None,
            y_title: None,
            source: None,
            by_rows: false,
            pivot: None,
        };

        let (read_back, bytes) = roundtrip_chart(sheet, combo_single);
        assert_eq!(read_back.kind, ChartKind::Combo, "Single series combo must retain Combo kind");
        assert_eq!(read_back.series.len(), 1);
        assert_eq!(read_back.series[0].kind, Some(ChartKind::ColumnClustered));

        let zip_text = std::io::Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(zip_text).expect("zip");
        let mut chart_xml = String::new();
        std::io::Read::read_to_string(&mut archive.by_name("xl/charts/chart1.xml").expect("chart1"), &mut chart_xml).expect("read");
        assert!(chart_xml.contains("barChart"), "Must contain primary barChart group");
        assert!(chart_xml.contains("lineChart"), "Must contain secondary lineChart dummy group for Combo");
    }
}
