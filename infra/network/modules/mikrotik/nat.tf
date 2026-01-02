# =============================================================================
# NAT Configuration
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ip_firewall_nat
# =============================================================================

# Masquerade outbound traffic from internal networks to WAN
resource "routeros_ip_firewall_nat" "masquerade" {
  for_each = { for k, v in var.ethernet_interfaces : k => v if v.wan_port }

  chain         = "srcnat"
  action        = "masquerade"
  out_interface = each.key
  comment       = "Masquerade outbound traffic to WAN"
}
