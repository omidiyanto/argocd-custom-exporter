use kube::{api::{ApiResource, GroupVersionKind, DynamicObject}, runtime::reflector};

fn main() {
    let gvk = GroupVersionKind::gvk("argoproj.io", "v1alpha1", "Application");
    let api_resource = ApiResource::from_gvk(&gvk);
    let writer = reflector::store::Writer::<DynamicObject>::new(api_resource);
    let store = writer.as_reader();
}
