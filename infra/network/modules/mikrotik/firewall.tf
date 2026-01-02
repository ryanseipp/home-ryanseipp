# =============================================================================
# Firewall Configuration
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ip_firewall_filter
# =============================================================================

locals {
  # Get WAN interface name (first one found, if any)
  # Use placeholder "none" when no WAN exists - rules referencing it won't be created anyway
  wan_interfaces = [for k, v in var.ethernet_interfaces : k if v.wan_port]
  has_wan        = length(local.wan_interfaces) > 0
  wan_interface  = local.has_wan ? local.wan_interfaces[0] : "none"

  # =========================================================================
  # IPv4 Filter Rules - Base (always applied)
  # =========================================================================
  ipv4_base_rules = {
    # Forward Chain - Global
    "fasttrack" = {
      chain            = "forward"
      action           = "fasttrack-connection"
      connection_state = "established,related"
      hw_offload       = true
      order            = 100
    }
    "accept-established-forward" = {
      chain            = "forward"
      action           = "accept"
      connection_state = "established,related"
      order            = 110
    }
    "drop-invalid-forward" = {
      chain            = "forward"
      action           = "drop"
      connection_state = "invalid"
      order            = 120
    }

    # Input Chain - Global
    "accept-icmp" = {
      chain    = "input"
      action   = "accept"
      protocol = "icmp"
      order    = 200
    }
    "accept-established-input" = {
      chain            = "input"
      action           = "accept"
      connection_state = "established,related"
      order            = 210
    }
    "drop-invalid-input" = {
      chain            = "input"
      action           = "drop"
      connection_state = "invalid"
      order            = 220
    }
  }

  # =========================================================================
  # IPv4 Filter Rules - WAN-specific (only on devices with WAN port)
  # =========================================================================
  ipv4_wan_rules = {
    "accept-internal-input" = {
      chain        = "input"
      action       = "accept"
      in_interface = "!${local.wan_interface}"
      order        = 300
    }

    # Forward Chain - Zone Rules
    "allow-trusted-to-wan" = {
      chain         = "forward"
      action        = "accept"
      src_address   = "10.0.1.0/24"
      out_interface = local.wan_interface
      order         = 1000
    }
    "block-mgmt-to-wan" = {
      chain         = "forward"
      action        = "drop"
      src_address   = "192.168.88.0/24"
      out_interface = local.wan_interface
      order         = 1100
    }

    # Default Deny
    "drop-wan-forward" = {
      chain            = "forward"
      action           = "drop"
      connection_state = "new"
      in_interface     = local.wan_interface
      order            = 8000
    }
    "drop-wan-input" = {
      chain        = "input"
      action       = "drop"
      in_interface = local.wan_interface
      order        = 9000
    }
  }

  # Merge base + WAN rules (WAN rules only if has_wan)
  ipv4_filter_rules = merge(
    local.ipv4_base_rules,
    local.has_wan ? local.ipv4_wan_rules : {}
  )

  # Transform to ordered map
  ipv4_rules_ordered = [
    for k, v in local.ipv4_filter_rules : merge(v, {
      key      = k
      sort_key = format("%04d-%s", v.order, k)
    })
  ]
  ipv4_rules_map = {
    for rule in local.ipv4_rules_ordered : rule.sort_key => rule
  }

  # =========================================================================
  # IPv6 Filter Rules - Base (always applied)
  # =========================================================================
  ipv6_base_rules = {
    # Forward Chain - Global
    "accept-established-forward" = {
      chain            = "forward"
      action           = "accept"
      connection_state = "established,related"
      order            = 100
    }
    "drop-invalid-forward" = {
      chain            = "forward"
      action           = "drop"
      connection_state = "invalid"
      order            = 110
    }

    # Input Chain - Global
    "accept-icmpv6" = {
      chain    = "input"
      action   = "accept"
      protocol = "icmpv6"
      order    = 200
    }
    "accept-established-input" = {
      chain            = "input"
      action           = "accept"
      connection_state = "established,related"
      order            = 210
    }
    "drop-invalid-input" = {
      chain            = "input"
      action           = "drop"
      connection_state = "invalid"
      order            = 220
    }
  }

  # =========================================================================
  # IPv6 Filter Rules - WAN-specific (only on devices with WAN port)
  # =========================================================================
  ipv6_wan_rules = {
    "accept-dhcpv6" = {
      chain        = "input"
      action       = "accept"
      protocol     = "udp"
      src_port     = "547"
      dst_port     = "546"
      in_interface = local.wan_interface
      order        = 230
    }
    "accept-internal-input" = {
      chain        = "input"
      action       = "accept"
      in_interface = "!${local.wan_interface}"
      order        = 300
    }

    # Default Deny
    "drop-wan-forward" = {
      chain            = "forward"
      action           = "drop"
      connection_state = "new"
      in_interface     = local.wan_interface
      order            = 8000
    }
    "drop-wan-input" = {
      chain        = "input"
      action       = "drop"
      in_interface = local.wan_interface
      order        = 9000
    }
  }

  # Merge base + WAN rules (WAN rules only if has_wan)
  ipv6_filter_rules = merge(
    local.ipv6_base_rules,
    local.has_wan ? local.ipv6_wan_rules : {}
  )

  # Transform to ordered map
  ipv6_rules_ordered = [
    for k, v in local.ipv6_filter_rules : merge(v, {
      key      = k
      sort_key = format("%04d-%s", v.order, k)
    })
  ]
  ipv6_rules_map = {
    for rule in local.ipv6_rules_ordered : rule.sort_key => rule
  }
}

# =============================================================================
# IPv4 Firewall Filter Rules
# =============================================================================
resource "routeros_ip_firewall_filter" "rules" {
  for_each = local.ipv4_rules_map

  comment          = each.value.key
  chain            = each.value.chain
  action           = each.value.action
  connection_state = lookup(each.value, "connection_state", null)
  in_interface     = lookup(each.value, "in_interface", null)
  out_interface    = lookup(each.value, "out_interface", null)
  protocol         = lookup(each.value, "protocol", null)
  src_port         = lookup(each.value, "src_port", null)
  dst_port         = lookup(each.value, "dst_port", null)
  src_address      = lookup(each.value, "src_address", null)
  dst_address      = lookup(each.value, "dst_address", null)
  hw_offload       = lookup(each.value, "hw_offload", null)

  lifecycle {
    create_before_destroy = true
  }
}

resource "routeros_move_items" "ipv4_firewall" {
  resource_path = "/ip/firewall/filter"
  sequence      = [for idx in sort(keys(local.ipv4_rules_map)) : routeros_ip_firewall_filter.rules[idx].id]
  depends_on    = [routeros_ip_firewall_filter.rules]
}

# =============================================================================
# IPv6 Firewall Filter Rules
# =============================================================================
resource "routeros_ipv6_firewall_filter" "rules" {
  for_each = local.ipv6_rules_map

  comment          = each.value.key
  chain            = each.value.chain
  action           = each.value.action
  connection_state = lookup(each.value, "connection_state", null)
  in_interface     = lookup(each.value, "in_interface", null)
  out_interface    = lookup(each.value, "out_interface", null)
  protocol         = lookup(each.value, "protocol", null)
  src_port         = lookup(each.value, "src_port", null)
  dst_port         = lookup(each.value, "dst_port", null)
  src_address      = lookup(each.value, "src_address", null)
  dst_address      = lookup(each.value, "dst_address", null)

  lifecycle {
    create_before_destroy = true
  }
}

resource "routeros_move_items" "ipv6_firewall" {
  resource_path = "/ipv6/firewall/filter"
  sequence      = [for idx in sort(keys(local.ipv6_rules_map)) : routeros_ipv6_firewall_filter.rules[idx].id]
  depends_on    = [routeros_ipv6_firewall_filter.rules]
}
