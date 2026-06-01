use std::collections::BTreeSet;

use pengu_mesh_core::lease_coverage_matrix;
use pengu_mesh_http::bootstrap_routes;
use pengu_mesh_mcp::core_tools;

#[test]
fn lease_matrix_covers_every_mcp_tool_and_http_route() {
    let matrix = lease_coverage_matrix();

    let matrix_tools = matrix
        .iter()
        .filter_map(|entry| entry.mcp_tool.as_deref())
        .collect::<BTreeSet<_>>();
    let defined_tools = core_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(matrix_tools, defined_tools);

    let matrix_routes = matrix
        .iter()
        .filter_map(|entry| entry.http_route.as_deref())
        .collect::<BTreeSet<_>>();
    let defined_routes = bootstrap_routes()
        .into_iter()
        .map(|route| route.route)
        .collect::<BTreeSet<_>>();
    assert_eq!(matrix_routes, defined_routes);
}
