# =================================================================================================
# BGP Template
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/routing_bgp_template
# =================================================================================================
resource "routeros_routing_bgp_template" "bgp_underlay" {
  name             = "bgp_underlay"
  as               = local.node.asn
  address_families = "ipv6"
  hold_time        = "30s"
  keepalive_time   = "10s"

  lifecycle {
    ignore_changes = [router_id]
  }
}

# =================================================================================================
# BGP Connection
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/routing_bgp_connection
# =================================================================================================
resource "routeros_routing_bgp_connection" "peer" {
  for_each = local.node.peers

  name             = "peer-${each.key}"
  templates        = [routeros_routing_bgp_template.bgp_underlay.name]
  as               = routeros_routing_bgp_template.bgp_underlay.as
  address_families = "ipv6"
  hold_time        = "30s"
  keepalive_time   = "10s"

  use_bfd  = true
  connect  = true
  listen   = true
  disabled = false

  local {
    role    = "ebgp"
    address = split("/", routeros_ipv6_address.peers[each.key].address)[0]
  }

  remote {
    address = cidrhost(routeros_ipv6_address.peers[each.key].address, var.nodes[each.key].mode == "spine" ? 0 : 1)
    as      = var.nodes[each.key].asn
  }

  output {
    redistribute = "connected"
  }

  lifecycle {
    ignore_changes = [router_id]
  }
}

# =================================================================================================
# BFD Configuration
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/routing_bgp_connection
# =================================================================================================
resource "routeros_routing_bfd_configuration" "bgp_underlay" {
  disabled   = false
  min_tx     = "100ms"
  min_rx     = "100ms"
  multiplier = 3
}
