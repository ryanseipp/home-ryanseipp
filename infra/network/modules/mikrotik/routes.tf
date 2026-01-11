# =============================================================================
# Static Routes
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ip_route
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ipv6_route
# =============================================================================

resource "routeros_ip_route" "extra" {
  for_each = var.extra_routes.ipv4

  dst_address = each.value.dst_address
  gateway     = each.value.gateway
  comment     = each.value.comment
}

resource "routeros_ipv6_route" "extra" {
  for_each = var.extra_routes.ipv6

  dst_address = each.value.dst_address
  gateway     = each.value.gateway
  comment     = each.value.comment
}
