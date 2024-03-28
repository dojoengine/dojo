use std::collections::HashMap;
use std::sync::Mutex;

use dojo_metrics_derive::Metrics;
use metrics::{
    with_local_recorder, Counter, Gauge, Histogram, Key, KeyName, Label, Metadata, Recorder,
    SharedString, Unit,
};
use once_cell::sync::Lazy;
use serial_test::serial;

#[allow(dead_code)]
#[derive(Metrics)]
#[metrics(scope = "metrics_custom")]
struct CustomMetrics {
    #[metric(skip)]
    skipped_field_a: u8,
    /// A gauge with doc comment description.
    gauge: Gauge,
    #[metric(rename = "second_gauge", describe = "A gauge with metric attribute description.")]
    gauge2: Gauge,
    #[metric(skip)]
    skipped_field_b: u16,
    /// Some doc comment
    #[metric(describe = "Metric attribute description will be preferred over doc comment.")]
    counter: Counter,
    #[metric(skip)]
    skipped_field_c: u32,
    #[metric(skip)]
    skipped_field_d: u64,
    /// A renamed histogram.
    #[metric(rename = "histogram")]
    histo: Histogram,
    #[metric(skip)]
    skipped_field_e: u128,
}

#[allow(dead_code)]
#[derive(Metrics)]
#[metrics(dynamic = true)]
struct DynamicScopeMetrics {
    #[metric(skip)]
    skipped_field_a: u8,
    /// A gauge with doc comment description.
    gauge: Gauge,
    #[metric(rename = "second_gauge", describe = "A gauge with metric attribute description.")]
    gauge2: Gauge,
    #[metric(skip)]
    skipped_field_b: u16,
    /// Some doc comment
    #[metric(describe = "Metric attribute description will be preferred over doc comment.")]
    counter: Counter,
    #[metric(skip)]
    skipped_field_c: u32,
    #[metric(skip)]
    skipped_field_d: u64,
    /// A renamed histogram.
    #[metric(rename = "histogram")]
    histo: Histogram,
    #[metric(skip)]
    skipped_field_e: u128,
}

static RECORDER: Lazy<TestRecorder> = Lazy::new(TestRecorder::new);

fn test_describe(scope: &str) {
    assert_eq!(RECORDER.metrics_len(), 4);

    let gauge = RECORDER.get_metric(&format!("{scope}.gauge"));
    assert!(gauge.is_some());
    assert_eq!(
        gauge.unwrap(),
        TestMetric {
            ty: TestMetricTy::Gauge,
            description: Some("A gauge with doc comment description.".to_owned()),
            labels: None,
        }
    );

    let second_gauge = RECORDER.get_metric(&format!("{scope}.second_gauge"));
    assert!(second_gauge.is_some());
    assert_eq!(
        second_gauge.unwrap(),
        TestMetric {
            ty: TestMetricTy::Gauge,
            description: Some("A gauge with metric attribute description.".to_owned()),
            labels: None,
        }
    );

    let counter = RECORDER.get_metric(&format!("{scope}.counter"));
    assert!(counter.is_some());
    assert_eq!(
        counter.unwrap(),
        TestMetric {
            ty: TestMetricTy::Counter,
            description: Some(
                "Metric attribute description will be preferred over doc comment.".to_owned()
            ),
            labels: None,
        }
    );

    let histogram = RECORDER.get_metric(&format!("{scope}.histogram"));
    assert!(histogram.is_some());
    assert_eq!(
        histogram.unwrap(),
        TestMetric {
            ty: TestMetricTy::Histogram,
            description: Some("A renamed histogram.".to_owned()),
            labels: None,
        }
    );
}

#[test]
#[serial]
fn describe_metrics() {
    with_local_recorder(&*RECORDER as &dyn Recorder, || {
        CustomMetrics::describe();
        test_describe("metrics_custom");
        RECORDER.clear();
    });
}

#[test]
#[serial]
fn describe_dynamic_metrics() {
    with_local_recorder(&*RECORDER as &dyn Recorder, || {
        let scope = "local_scope";
        DynamicScopeMetrics::describe(scope);
        test_describe(scope);
        RECORDER.clear();
    });
}

fn test_register(scope: &str) {
    assert_eq!(RECORDER.metrics_len(), 4);

    let gauge = RECORDER.get_metric(&format!("{scope}.gauge"));
    assert!(gauge.is_some());
    assert_eq!(
        gauge.unwrap(),
        TestMetric { ty: TestMetricTy::Gauge, description: None, labels: None }
    );

    let second_gauge = RECORDER.get_metric(&format!("{scope}.second_gauge"));
    assert!(second_gauge.is_some());
    assert_eq!(
        second_gauge.unwrap(),
        TestMetric { ty: TestMetricTy::Gauge, description: None, labels: None }
    );

    let counter = RECORDER.get_metric(&format!("{scope}.counter"));
    assert!(counter.is_some());
    assert_eq!(
        counter.unwrap(),
        TestMetric { ty: TestMetricTy::Counter, description: None, labels: None }
    );

    let histogram = RECORDER.get_metric(&format!("{scope}.histogram"));
    assert!(histogram.is_some());
    assert_eq!(
        histogram.unwrap(),
        TestMetric { ty: TestMetricTy::Histogram, description: None, labels: None }
    );
}

#[test]
#[serial]
fn register_metrics() {
    with_local_recorder(&*RECORDER as &dyn Recorder, || {
        let _metrics = CustomMetrics::default();
        test_register("metrics_custom");
        RECORDER.clear();
    });
}

#[test]
#[serial]
fn register_dynamic_metrics() {
    with_local_recorder(&*RECORDER as &dyn Recorder, || {
        let scope = "local_scope";
        let _metrics = DynamicScopeMetrics::new(scope);
        test_register(scope);
        RECORDER.clear();
    });
}

fn test_labels(scope: &str) {
    let test_labels = vec![Label::new("key", "value")];

    let gauge = RECORDER.get_metric(&format!("{scope}.gauge"));
    assert!(gauge.is_some());
    let labels = gauge.unwrap().labels;
    assert!(labels.is_some());
    assert_eq!(labels.unwrap(), test_labels,);

    let second_gauge = RECORDER.get_metric(&format!("{scope}.second_gauge"));
    assert!(second_gauge.is_some());
    let labels = second_gauge.unwrap().labels;
    assert!(labels.is_some());
    assert_eq!(labels.unwrap(), test_labels,);

    let counter = RECORDER.get_metric(&format!("{scope}.counter"));
    assert!(counter.is_some());
    let labels = counter.unwrap().labels;
    assert!(labels.is_some());
    assert_eq!(labels.unwrap(), test_labels,);

    let histogram = RECORDER.get_metric(&format!("{scope}.histogram"));
    assert!(histogram.is_some());
    let labels = histogram.unwrap().labels;
    assert!(labels.is_some());
    assert_eq!(labels.unwrap(), test_labels,);
}

#[test]
#[serial]
fn label_metrics() {
    with_local_recorder(&*RECORDER as &dyn Recorder, || {
        let _metrics = CustomMetrics::new_with_labels(&[("key", "value")]);
        test_labels("metrics_custom");
        RECORDER.clear();
    });
}

#[test]
#[serial]
fn dynamic_label_metrics() {
    with_local_recorder(&*RECORDER as &dyn Recorder, || {
        let scope = "local_scope";
        let _metrics = DynamicScopeMetrics::new_with_labels(scope, &[("key", "value")]);
        test_labels(scope);
        RECORDER.clear();
    });
}

struct TestRecorder {
    // Metrics map: key => Option<description>
    metrics: Mutex<HashMap<String, TestMetric>>,
}

#[derive(PartialEq, Clone, Debug)]
enum TestMetricTy {
    Counter,
    Gauge,
    Histogram,
}

#[derive(PartialEq, Clone, Debug)]
struct TestMetric {
    ty: TestMetricTy,
    description: Option<String>,
    labels: Option<Vec<Label>>,
}

impl TestRecorder {
    fn new() -> Self {
        Self { metrics: Mutex::new(HashMap::default()) }
    }

    fn metrics_len(&self) -> usize {
        self.metrics.lock().expect("failed to lock metrics").len()
    }

    fn get_metric(&self, key: &str) -> Option<TestMetric> {
        self.metrics.lock().expect("failed to lock metrics").get(key).cloned()
    }

    fn record_metric(
        &self,
        key: &str,
        ty: TestMetricTy,
        description: Option<String>,
        labels: Option<Vec<Label>>,
    ) {
        self.metrics
            .lock()
            .expect("failed to lock metrics")
            .insert(key.to_owned(), TestMetric { ty, description, labels });
    }

    fn clear(&self) {
        self.metrics.lock().expect("failed to lock metrics").clear();
    }
}

impl Recorder for TestRecorder {
    fn describe_counter(&self, key: KeyName, _unit: Option<Unit>, description: SharedString) {
        self.record_metric(
            key.as_str(),
            TestMetricTy::Counter,
            Some(description.into_owned()),
            None,
        )
    }

    fn describe_gauge(&self, key: KeyName, _unit: Option<Unit>, description: SharedString) {
        self.record_metric(key.as_str(), TestMetricTy::Gauge, Some(description.into_owned()), None)
    }

    fn describe_histogram(&self, key: KeyName, _unit: Option<Unit>, description: SharedString) {
        self.record_metric(
            key.as_str(),
            TestMetricTy::Histogram,
            Some(description.into_owned()),
            None,
        )
    }

    fn register_counter(&self, key: &Key, _: &Metadata<'_>) -> Counter {
        let labels_vec: Vec<Label> = key.labels().cloned().collect();
        let labels = (!labels_vec.is_empty()).then_some(labels_vec);
        self.record_metric(key.name(), TestMetricTy::Counter, None, labels);
        Counter::noop()
    }

    fn register_gauge(&self, key: &Key, _: &Metadata<'_>) -> Gauge {
        let labels_vec: Vec<Label> = key.labels().cloned().collect();
        let labels = (!labels_vec.is_empty()).then_some(labels_vec);
        self.record_metric(key.name(), TestMetricTy::Gauge, None, labels);
        Gauge::noop()
    }

    fn register_histogram(&self, key: &Key, _: &Metadata<'_>) -> Histogram {
        let labels_vec: Vec<Label> = key.labels().cloned().collect();
        let labels = (!labels_vec.is_empty()).then_some(labels_vec);
        self.record_metric(key.name(), TestMetricTy::Histogram, None, labels);
        Histogram::noop()
    }
}
